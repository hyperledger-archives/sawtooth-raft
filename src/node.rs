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

use std::time::Instant;

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
    engine::{Block, BlockId, PeerMessage, Error},
    service::Service,
};

use config;

/// Possible states a leader node can be in
///
/// The leader transitions between the following states in order:
///   Building - The leader is building a new block
///   Publishing - The leader has built a new block and it is being published
///   Validating - The leader is validating the block it published
///   Proposing - The leader is proposing the valid block to its peers
///   Committing - The leader is committing the block that has been accepted by its peers
enum LeaderState {
    Building(Instant), // Instant is when the block started being built
    Publishing(BlockId),
    Validating(BlockId),
    Proposing(BlockId),
    Committing(BlockId),
}

// TODO: add peers
pub struct SawtoothRaftNode {
    raw_node: RawNode<MemStorage>,
    service: Box<Service>,
    leader_state: Option<LeaderState>,
}

impl SawtoothRaftNode {
    pub fn new(raw_node: RawNode<MemStorage>, service: Box<Service>) -> Self {
        SawtoothRaftNode {
            raw_node,
            service,
            leader_state: None,
        }
    }

    pub fn on_block_new(&mut self, block: Block) {
        trace!("Received new block: {:?}", block);

        if match self.leader_state {
            Some(LeaderState::Publishing(ref block_id)) => {
                block_id == &block.block_id
            },
            _ => false,
        } {
            self.leader_state = Some(LeaderState::Validating(block.block_id.clone()));
        }

        // TODO: Only check blocks that we expect to get
        self.service.check_blocks(vec![block.block_id]).expect("Failed to send check blocks");
    }

    pub fn on_block_valid(&mut self, block_id: BlockId) {
        trace!("Block would commit: {:?}", block_id);
        // Handle out of order new/valid updates
        if match self.leader_state {
            Some(LeaderState::Publishing(ref expected)) => {
                expected == &block_id
            },
            Some(LeaderState::Validating(ref expected)) => {
                expected == &block_id
            },
            _ => false,
        } {
            self.raw_node.propose(vec![], block_id.clone().into()).expect("Failed to propose block to Raft");
            self.leader_state = Some(LeaderState::Proposing(block_id));
        }
    }

    pub fn on_block_commit(&mut self, block_id: BlockId) {
        trace!("Block committed: {:?}", block_id);
        if match self.leader_state {
            Some(LeaderState::Committing(ref committing)) => {
                committing == &block_id
            },
            _ => false,
        } {
            self.leader_state = Some(LeaderState::Building(Instant::now()));
            self.service.initialize_block(None).expect("Failed to initialize block");
        }
    }

    pub fn on_peer_message(&mut self, message: PeerMessage) {
        let raft_message = try_into_raft_message(&message)
            .expect("Failed to interpret bytes as Raft message");
        self.raw_node.step(raft_message)
            .expect("Failed to handle Raft message");
    }

    pub fn tick(&mut self) {
        self.raw_node.tick();
        self.check_publish();
    }

    fn check_publish(&mut self) {
        // We want to publish a block if:
        // 1. We are the leader
        // 2. We are building a block
        // 3. The block has been building long enough
        if match self.leader_state {
            Some(LeaderState::Building(instant)) => {
                instant.elapsed() >= config::PUBLISH_PERIOD
            }
            _ => false,
        } {
            match self.service.finalize_block(vec![]) {
                Ok(block_id) => self.leader_state = Some(LeaderState::Publishing(block_id)),
                Err(Error::BlockNotReady) => (), // Try again later
                Err(err) => panic!("Failed to finalize block: {:?}", err),
            };
        }
    }

    pub fn process_ready(&mut self) {
        if !self.raw_node.has_ready() {
            return
        }

        // The Raft is ready, we can do something now.
        let mut ready = self.raw_node.ready();

        let is_leader = self.raw_node.raft.state == raft::StateRole::Leader;
        if is_leader {
            // We just became the leader, so we need to start building a block
            if self.leader_state.is_none() {
                self.leader_state = Some(LeaderState::Building(Instant::now()));
                self.service.initialize_block(None).expect("Failed to initialize block");
            }
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
            self.raw_node.mut_store()
                .wl()
                .apply_snapshot(ready.snapshot.clone())
                .unwrap();
        }

        if !ready.entries.is_empty() {
            // Append entries to the Raft log
            self.raw_node.mut_store().wl().append(&ready.entries).unwrap();
        }

        if let Some(ref hs) = ready.hs {
            // Raft HardState changed, and we need to persist it.
            self.raw_node.mut_store().wl().set_hardstate(hs.clone());
        }

        if !is_leader {
            // We just stepped down as leader
            if self.leader_state.is_some() {
                // If we were building a block, cancel it
                match self.leader_state {
                    Some(LeaderState::Building(_)) =>
                        self.service.cancel_block().expect("Failed to cancel block"),
                    _ => (),
                }
                self.leader_state = None;
            }
            // If not leader, the follower needs to reply the messages to
            // the leader after appending Raft entries.
            let msgs = ready.messages.drain(..);
            for msg in msgs {
                // Send messages to other peers.
                let peer_msg = try_into_peer_message(&msg)
                    .expect("Failed to convert into peer message");
                self.service.send_to()
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
                    // TODO: This usage of Raft only proposes one block at a time. Performance
                    // could be improved by optimistically proposing blocks before parents are
                    // committed.
                    trace!("Entry committed: {:?}", entry);
                    let block_id: BlockId = BlockId::from(Vec::from(entry.get_data()));
                    if match self.leader_state {
                        Some(LeaderState::Proposing(ref proposed)) => {
                            &block_id == proposed
                        },
                        _ => false,
                    } {
                        self.service.commit_block(block_id.clone()).expect("Failed to commit block");
                        self.leader_state = Some(LeaderState::Committing(block_id));
                    }
                }

                // TODO: handle EntryConfChange
            }
        }

        // Advance the Raft
        self.raw_node.advance(ready);
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
