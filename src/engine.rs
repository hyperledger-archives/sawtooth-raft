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

use std::time::Duration;
use std::sync::mpsc::{Receiver, RecvTimeoutError};

use raft::{
    raw_node::RawNode,
    Peer as RaftPeer,
};

use sawtooth_sdk::consensus::{
    engine::{Block, PeerInfo, Engine, Update},
    service::Service,
};

use config::{self, RaftEngineConfig};
use ticker;
use node::SawtoothRaftNode;


pub struct RaftEngine {
    id: u64,
}

impl RaftEngine {
    pub fn new(id: u64) -> Self {
        RaftEngine { id }
    }
}

pub const RAFT_TIMEOUT: Duration = Duration::from_millis(100);

impl Engine for RaftEngine {
    fn start(
        &mut self,
        updates: Receiver<Update>,
        mut service: Box<Service>,
        chain_head: Block,
        _peers: Vec<PeerInfo>,
    ) {

        // Create the configuration for the Raft node.
        let RaftEngineConfig {
            peers,
            period,
            raft: raft_config,
            storage: raft_storage
        } = config::load_raft_config(self.id, chain_head.block_id, &mut service);

        let raft_peers: Vec<RaftPeer> = peers
            .values()
            .map(|id| RaftPeer { id: *id, context: None })
            .collect();
        // Create the Raft node.
        let raw_node = RawNode::new(&raft_config, raft_storage, raft_peers).unwrap();

        let mut node = SawtoothRaftNode::new(raw_node, service, peers, period);

        let raft_timeout = RAFT_TIMEOUT;
        let mut raft_ticker = ticker::Ticker::new(raft_timeout);
        let mut timeout = raft_timeout;

        // Loop forever to drive the Raft.
        loop {
            trace!("Top of main loop");
            match updates.recv_timeout(timeout) {
                Ok(Update::BlockNew(block)) => node.on_block_new(block),
                Ok(Update::BlockValid(block_id)) => node.on_block_valid(block_id),
                Ok(Update::BlockCommit(block_id)) => node.on_block_commit(block_id),
                Ok(Update::PeerMessage(message, _id)) => node.on_peer_message(message),
                Ok(Update::Shutdown) => return,
                Err(RecvTimeoutError::Timeout) => (),
                Err(RecvTimeoutError::Disconnected) => return,
                // TODO: Handle invalid, peer update
                _ => unimplemented!(),
            }

            timeout = raft_ticker.tick(|| {
                node.tick();
            });

            node.process_ready();
        }
    }

    fn version(&self) -> String {
        "0.1".into()
    }

    fn name(&self) -> String {
        "Raft".into()
    }
}
