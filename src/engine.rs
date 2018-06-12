/*
 * Copyright 2018 Intel Corporation
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 * ------------------------------------------------------------------------------
 */

use std::cmp;
use std::sync::mpsc::{Receiver, RecvTimeoutError};

use protobuf::{self, Message as ProtobufMessage, ProtobufError};
use raft::{
    self,
    eraftpb::{
        EntryType,
        Message as RaftMessage,
    },
    raw_node::RawNode,
    storage::MemStorage,
};

use sawtooth_sdk::consensus::{
    engine::{Block, BlockId, PeerInfo, PeerMessage, Engine, Error, Update},
    service::Service,
};

use config;
use ticker;


pub struct RaftEngine {
    raft_node: RawNode<MemStorage>,
}

impl RaftEngine {
    pub fn new(raft_node: RawNode<MemStorage>) -> Self {
        RaftEngine { raft_node }
    }
}

impl Engine for RaftEngine {
    fn start(
        &mut self,
        updates: Receiver<Update>,
        mut service: Box<Service>,
        _chain_head: Block,
        _peers: Vec<PeerInfo>,
    ) {
        let raft_timeout = config::RAFT_PERIOD;
        let publish_timeout = config::PUBLISH_PERIOD;

        let mut raft_ticker = ticker::Ticker::new(raft_timeout);
        let mut publish_ticker = ticker::Ticker::new(publish_timeout);

        let mut timeout = cmp::min(raft_timeout, publish_timeout);

        trace!("Initializing first block");
        service.initialize_block(None).expect("Initialize block failed");

        // Loop forever to drive the Raft.
        loop {
            trace!("Top of main loop");
            match updates.recv_timeout(timeout) {
                // Propose is the equivalent of publish block
                Ok(Update::BlockNew(block)) => self.on_block_new(block, &mut service),
                Ok(Update::BlockValid(block_id)) => self.on_block_valid(block_id),
                Ok(Update::BlockCommit(block_id)) => self.on_block_commit(block_id),
                // This is a consensus message that should be passed to the node
                Ok(Update::PeerMessage(message, _id)) => self.on_peer_message(message),
                Err(RecvTimeoutError::Timeout) => (),
                Err(RecvTimeoutError::Disconnected) => return,
                _ => unimplemented!(),
            }

            let raft_timeout = raft_ticker.tick(|| {
                self.raft_node.tick();
            });
            let publish_timeout = publish_ticker.tick(|| {
                self.propose_block(&mut service)
            });
            timeout = cmp::min(raft_timeout, publish_timeout);

            if self.raft_node.has_ready() {
                self.on_ready(&mut service);
            }
        }
    }

    fn version(&self) -> String {
        "0.1".into()
    }

    fn name(&self) -> String {
        "Raft".into()
    }
}

impl RaftEngine {
    fn on_block_new(&mut self, block: Block, service: &mut Box<Service>) {
        trace!("Received new block: {:?}", block);
        service.check_blocks(vec![block.block_id]).expect("Failed to send check blocks");
    }

    fn on_block_valid(&mut self, block_id: BlockId) {
        trace!("Block will commit: {:?}", block_id);
        self.raft_node.propose(vec![], block_id.into()).expect("Failed to propose block to Raft");
    }

    fn on_block_commit(&mut self, block_id: BlockId) {
        trace!("Block committed: {:?}", block_id);
    }

    fn on_peer_message(&mut self, message: PeerMessage) {
        let raft_message = try_into_raft_message(&message)
            .expect("Failed to interpret bytes as Raft message");
        self.raft_node.step(raft_message)
            .expect("Failed to handle Raft message");
    }

    fn propose_block(&self, service: &mut Box<Service>) {
        match self.raft_node.raft.state {
            raft::StateRole::Leader => {
                match service.finalize_block(vec![]) {
                    Ok(block_id) => {
                        println!("Published block '{:?}'", block_id);
                        service.initialize_block(None).expect("Initialize block failed");
                    },
                    Err(Error::BlockNotReady) => (),
                    Err(err) => panic!("Failed to initialize block: {:?}", err),
                }
            },
            _ => (),
        }
    }

    fn on_ready(&mut self, service: &mut Box<Service>) {
        // The Raft is ready, we can do something now.
        let mut ready = self.raft_node.ready();

        let is_leader = self.raft_node.raft.leader_id == 1;
        if is_leader {
            // If the peer is leader, the leader can send messages to other followers ASAP.
            let msgs = ready.messages.drain(..);
            for msg in msgs {
                let _peer_msg = try_into_peer_message(&msg)
                    .expect("Failed to convert into peer message");
                // TODO: Send to peer
            }
        }

        if !raft::is_empty_snap(&ready.snapshot) {
            // This is a snapshot, we need to apply the snapshot at first.
            self.raft_node.mut_store()
                .wl()
                .apply_snapshot(ready.snapshot.clone())
                .unwrap();
        }

        if !ready.entries.is_empty() {
            // Append entries to the Raft log
            self.raft_node.mut_store().wl().append(&ready.entries).unwrap();
        }

        if let Some(ref hs) = ready.hs {
            // Raft HardState changed, and we need to persist it.
            self.raft_node.mut_store().wl().set_hardstate(hs.clone());
        }

        if !is_leader {
            // If not leader, the follower needs to reply the messages to
            // the leader after appending Raft entries.
            let msgs = ready.messages.drain(..);
            for msg in msgs {
                // Send messages to other peers.
                let _peer_msg = try_into_peer_message(&msg)
                    .expect("Failed to convert into peer message");
                // TODO: Send to peer
            }
        }

        if let Some(committed_entries) = ready.committed_entries.take() {
            let mut _last_apply_index = 0;
            for entry in committed_entries {
                // Mostly, you need to save the last apply index to resume applying
                // after restart. Here we just ignore this because we use a Memory storage.
                _last_apply_index = entry.get_index();

                if entry.get_data().is_empty() {
                    // Emtpy entry, when the peer becomes Leader it will send an empty entry.
                    continue;
                }

                if entry.get_entry_type() == EntryType::EntryNormal {
                    trace!("Entry committed: {:?}", entry);
                    let block_id: BlockId = BlockId::from(Vec::from(entry.get_data()));
                    service.commit_block(block_id).expect("Failed to commit block");
                }

                // TODO: handle EntryConfChange
            }
        }

        // Advance the Raft
        self.raft_node.advance(ready);
    }
}

fn try_into_raft_message(message: &PeerMessage) -> Result<RaftMessage, ProtobufError> {
    protobuf::parse_from_bytes(&message.content)
}

fn try_into_peer_message(message: &RaftMessage) -> Result<PeerMessage, ProtobufError> {
    Ok(PeerMessage {
        message_type: String::new(),
        content: message.write_to_bytes()?,
    })
}
