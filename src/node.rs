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

use std::collections::{HashMap, HashSet};
use std::iter::FromIterator;
use std::time::{Duration, Instant};

use protobuf::{self, Message as ProtobufMessage, ProtobufError};
use raft::{
    self,
    eraftpb::{
        ConfChange,
        ConfChangeType,
        EntryType,
        Message as RaftMessage,
    },
    raw_node::RawNode,
};

use sawtooth_sdk::consensus::{
    engine::{Block, BlockId, PeerId, PeerMessage, Error},
    service::Service,
};

use block_queue::BlockQueue;
use storage::StorageExt;
use config::{peer_id_to_raft_id, get_peers_from_settings};

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
    ChangingConfig,
}

enum FollowerState {
    Idle,
    Committing(BlockId),
}

pub struct SawtoothRaftNode<S: StorageExt> {
    peer_id: PeerId,
    raw_node: RawNode<S>,
    service: Box<Service>,
    leader_state: Option<LeaderState>,
    follower_state: Option<FollowerState>,
    block_queue: BlockQueue,
    raft_id_to_peer_id: HashMap<u64, PeerId>,
    period: Duration,
}

impl<S: StorageExt> SawtoothRaftNode<S> {
    pub fn new(
        peer_id: PeerId,
        raw_node: RawNode<S>,
        service: Box<Service>,
        peers: Vec<PeerId>,
        period: Duration
    ) -> Self {
        SawtoothRaftNode {
            peer_id,
            raw_node,
            service,
            leader_state: None,
            follower_state: Some(FollowerState::Idle),
            block_queue: BlockQueue::new(),
            raft_id_to_peer_id: peers.into_iter().map(|peer_id| (peer_id_to_raft_id(&peer_id), peer_id)).collect(),
            period,
        }
    }

    pub fn on_block_new(&mut self, block: Block) {
        if match self.leader_state {
            Some(LeaderState::Publishing(ref block_id)) => {
                block_id == &block.block_id
            },
            _ => false,
        } {
            debug!("Leader({:?}) transition to Validating block {:?}", self.peer_id, block.block_id);
            self.leader_state = Some(LeaderState::Validating(block.block_id.clone()));
        }

        debug!("Block has been received: {:?}", &block);
        self.block_queue.block_new(block.clone());

        self.service.check_blocks(vec![block.block_id]).expect("Failed to send check blocks");
    }

    pub fn on_block_valid(&mut self, block_id: BlockId) {
        // Handle out of order new/valid updates
        debug!("Block has been validated: {:?}", &block_id);
        self.block_queue.block_valid(&block_id);
        if match self.leader_state {
            Some(LeaderState::Publishing(ref expected)) => {
                expected == &block_id
            },
            Some(LeaderState::Validating(ref expected)) => {
                expected == &block_id
            },
            _ => false,
        } {
            debug!("Leader({:?}) transition to Proposing block {:?}", self.peer_id, block_id);
            info!("Leader({:?}) proposed block {:?}", self.peer_id, block_id);
            self.raw_node.propose(vec![], block_id.clone().into()).expect("Failed to propose block to Raft");
            self.leader_state = Some(LeaderState::Proposing(block_id));
        }
    }

    pub fn on_block_commit(&mut self, block_id: BlockId) {
        if match self.leader_state {
            Some(LeaderState::Committing(ref committing)) => {
                committing == &block_id
            },
            _ => false,
        } {
            let entry = self.block_queue.block_committed();
            self.raw_node.raft.mut_store().set_applied(entry).expect("Failed to set last applied entry.");

            if let Some(change) = self.check_for_conf_change(block_id.clone()) {
                debug!("Leader({:?}) transition to ChangingConfig", self.peer_id);
                self.leader_state = Some(LeaderState::ChangingConfig);
                self.raw_node.propose_conf_change(vec![], change).unwrap();
            } else {
                debug!("Leader({:?}) transition to Building block {:?}", self.peer_id, block_id);
                self.leader_state = Some(LeaderState::Building(Instant::now()));
                self.service.initialize_block(None).expect("Failed to initialize block");
            }
        }

        if match self.follower_state {
            Some(FollowerState::Committing(ref committing)) => {
                committing == &block_id
            },
            _ => false,
        } {
            info!("Peer({:?}) committed block {:?}", self.peer_id, block_id);
            let entry = self.block_queue.block_committed();
            self.raw_node.raft.mut_store().set_applied(entry).expect("Failed to set last applied entry.");
            self.follower_state = Some(FollowerState::Idle);
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
                instant.elapsed() >= self.period
            }
            _ => false,
        } {
            match self.service.finalize_block(vec![]) {
                Ok(block_id) => {
                    debug!("Leader({:?}) transition to Publishing block {:?}", self.peer_id, block_id);
                    self.leader_state = Some(LeaderState::Publishing(block_id));
                },
                Err(Error::BlockNotReady) => {
                     // Try again later
                    debug!("Leader({:?}) tried to finalize block but block not ready", self.peer_id);
                },
                Err(err) => panic!("Failed to finalize block: {:?}", err),
            };
        }
    }

    fn send_msg(&mut self, raft_msg: &RaftMessage) {
        let peer_msg = try_into_peer_message(raft_msg)
            .expect("Failed to convert into peer message");
        if let Some(peer_id) = self.raft_id_to_peer_id.get(&raft_msg.to) {
            match self.service.send_to(
                peer_id,
                "",
                peer_msg.content,
            ) {
                Ok(_) => (),
                Err(Error::UnknownPeer(s)) => warn!("Tried to send to disconnected peer: {}", s),
                Err(err) => panic!("Failed to send to peer: {:?}", err),
            }
        } else {
            warn!("Tried to send to unknown peer: {}", raft_msg.to);
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
                debug!("Leader({:?}) became leader, intializing block", self.peer_id);
                self.follower_state = None;
                self.leader_state = Some(LeaderState::Building(Instant::now()));
                self.service.initialize_block(None).expect("Failed to initialize block");
            }
            // If the peer is leader, the leader can send messages to other followers ASAP.
            for msg in ready.messages.drain(..) {
                debug!("Leader({:?}) wants to send message: {:?}", self.peer_id, msg);
                self.send_msg(&msg);
            }
        }

        if !raft::is_empty_snap(&ready.snapshot) {
            // This is a snapshot, we need to apply the snapshot at first.
            self.raw_node.mut_store()
                .apply_snapshot(&ready.snapshot)
                .unwrap();
        }

        if !ready.entries.is_empty() {
            // Append entries to the Raft log
            self.raw_node.mut_store().append(&ready.entries).expect("Failed to append entries");
        }

        if let Some(ref hs) = ready.hs {
            // Raft HardState changed, and we need to persist it.
            self.raw_node.mut_store().set_hardstate(hs);
        }

        if !is_leader {
            // We just stepped down as leader
            if self.leader_state.is_some() {
                // If we were building a block, cancel it
                match self.leader_state {
                    Some(LeaderState::Building(_)) => {
                        debug!("Leader({:?}) stepped down, cancelling block", self.peer_id);
                        self.service.cancel_block().expect("Failed to cancel block");
                    }
                    Some(LeaderState::Committing(ref block_id)) => {
                        self.follower_state = Some(FollowerState::Committing(block_id.clone()));
                    },
                    _ => (),
                }
                self.leader_state = None;
                if self.follower_state.is_none() {
                    self.follower_state = Some(FollowerState::Idle);
                }
            }
            // If not leader, the follower needs to reply the messages to
            // the leader after appending Raft entries.
            let msgs = ready.messages.drain(..);
            for msg in msgs {
                debug!("Peer({:?}) wants to send message: {:?}", self.peer_id, msg);
                self.send_msg(&msg);
            }
        }

        if let Some(committed_entries) = ready.committed_entries.take() {
            for entry in committed_entries {
                if entry.get_data().is_empty() {
                    // When the peer becomes Leader it will send an empty entry.
                    continue;
                }

                if entry.get_entry_type() == EntryType::EntryNormal {
                    let block_id: BlockId = BlockId::from(Vec::from(entry.get_data()));
                    self.block_queue.add_block_commit(block_id, entry.get_index());
                } else if entry.get_entry_type() == EntryType::EntryConfChange && entry.get_term() != 1 {
                    let change: ConfChange = protobuf::parse_from_bytes(entry.get_data())
                        .expect("Failed to parse ConfChange");

                    self.apply_conf_change(&change);

                    if let Some(LeaderState::ChangingConfig) = self.leader_state {
                        debug!("Leader({:?}) transition to Building block", self.peer_id);
                        self.leader_state = Some(LeaderState::Building(Instant::now()));
                        self.service.initialize_block(None).expect("Failed to initialize block");
                    }
                }
            }
        }

        // Commit next committable block from backlog (if there is one)
        let chain_head = self.service
            .get_chain_head()
            .expect("Chain head should not be none");
        if let Some(block_id) = self.block_queue.get_next_committable(&chain_head) {
            self.commit_block(&block_id);
        };

        // Advance the Raft
        self.raw_node.advance(ready);
    }

    fn commit_block(&mut self, block_id: &BlockId) {
        if match self.leader_state {
            Some(LeaderState::Proposing(ref proposed)) => {
                block_id == proposed
            },
            _ => false,
        } {
            debug!("Leader({:?}) transitioning to Committing block {:?}", self.peer_id, block_id);
            self.leader_state = Some(LeaderState::Committing(block_id.clone()));
            self.service.commit_block(block_id.clone()).expect("Failed to commit block");
        }

        if let Some(FollowerState::Idle) = self.follower_state {
            debug!("Peer({:?}) committing block {:?}", self.peer_id, block_id);
            self.follower_state = Some(FollowerState::Committing(block_id.clone()));
            self.service.commit_block(block_id.clone()).expect("Failed to commit block");
        }
    }

    fn check_for_conf_change(&mut self, block_id: BlockId) -> Option<ConfChange> {
        // Get list of peers from settings
        let settings = self.service
            .get_settings(block_id, vec![String::from("sawtooth.consensus.raft.peers")])
            .expect("Failed to get settings");
        let peers = get_peers_from_settings(&settings);
        let peers: HashSet<PeerId> = HashSet::from_iter(peers);

        // Check if membership has changed
        let old_peers: HashSet<PeerId> = HashSet::from_iter(self.raft_id_to_peer_id.values().cloned());
        let difference: HashSet<PeerId> = peers.symmetric_difference(&old_peers).cloned().collect();

        if !difference.is_empty() {
            if difference.len() > 1 {
                panic!("More than one node's membership has changed; only one change can be processed at a time.");
            }

            // The raft-rs library can only add or delete one node at a time, so there should
            // only be one item in this iterator
            let peer_id = difference.iter().nth(0).expect("Difference cannot be empty here.");
            // Initialize change
            let mut change = ConfChange::new();
            change.set_node_id(peer_id_to_raft_id(&peer_id));

            // Check if adding or removing Node
            if peers.len() > old_peers.len() {
                change.set_change_type(ConfChangeType::AddNode);
                // Include peer ID in context so node can be added to raft_id_to_peer_id map later
                change.set_context(Vec::from(peer_id.clone()));
            } else if peers.len() < old_peers.len() {
                change.set_change_type(ConfChangeType::RemoveNode);
            }

            info!("Leader({:?}) detected configuration change {:?}", self.peer_id, change);
            return Some(change);
        }

        None
    }

    fn apply_conf_change(&mut self, change: &ConfChange) {
        info!("Configuration change received: {:?}", change);
        let raft_id = change.get_node_id();

        // Update raft_id_to_peer_id according to change
        match change.get_change_type() {
            ConfChangeType::RemoveNode => {
                self.raft_id_to_peer_id.remove(&raft_id).unwrap();
            },
            ConfChangeType::AddNode => {
                self.raft_id_to_peer_id.insert(
                    raft_id,
                    PeerId::from(Vec::from(change.get_context()))
                );
            },
            _ => {}
        }

        self.raw_node.apply_conf_change(&change);
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
