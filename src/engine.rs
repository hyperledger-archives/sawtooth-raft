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
    engine::{StartupState, Engine, Update},
    service::Service,
};

use config::{self, RaftEngineConfig};
use ticker;
use node::SawtoothRaftNode;
use storage::StorageExt;


pub struct RaftEngine {}

impl RaftEngine {
    pub fn new() -> Self {
        RaftEngine {}
    }
}

pub const RAFT_TIMEOUT: Duration = Duration::from_millis(100);

impl Engine for RaftEngine {
    fn start(
        &mut self,
        updates: Receiver<Update>,
        mut service: Box<Service>,
        startup_state: StartupState,
    ) {
        let StartupState {
            chain_head,
            local_peer_info,
            ..
        } = startup_state;

        // Create the configuration for the Raft node.
        let cfg = config::load_raft_config(
            &local_peer_info.peer_id,
            chain_head.block_id,
            &mut service
        );
        info!("Raft Engine Config Loaded: {:?}", cfg);
        let RaftEngineConfig {
            peers,
            period,
            raft: raft_config,
            storage: raft_storage
        } = cfg;

        // Create the Raft node.
        let raft_peers: Vec<RaftPeer> = raft_config.peers
            .iter()
            .map(|id| RaftPeer { id: *id, context: None })
            .collect();
        let raw_node = RawNode::new(
            &raft_config,
            raft_storage,
            raft_peers
        ).expect("Failed to create new RawNode");

        let mut node = SawtoothRaftNode::new(
            local_peer_info.peer_id,
            raw_node,
            service,
            peers,
            period
        );

        let mut raft_ticker = ticker::Ticker::new(RAFT_TIMEOUT);
        let mut timeout = RAFT_TIMEOUT;

        // Loop forever to drive the Raft.
        loop {
            match updates.recv_timeout(timeout) {
                Err(RecvTimeoutError::Timeout) => (),
                Err(RecvTimeoutError::Disconnected) => break,
                Ok(update) => {
                    debug!("Update: {:?}", update);
                    if !handle_update(&mut node, update) {
                        break;
                    }
                }
            }

            timeout = raft_ticker.tick(|| {
                node.tick();
            });

            node.process_ready();
        }
    }

    fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").into()
    }

    fn name(&self) -> String {
        env!("CARGO_PKG_NAME").into()
    }
}

// Returns whether the engine should continue
fn handle_update<S: StorageExt>(node: &mut SawtoothRaftNode<S>, update: Update) -> bool {
    match update {
        Update::BlockNew(block) => node.on_block_new(block),
        Update::BlockValid(block_id) => node.on_block_valid(block_id),
        Update::BlockCommit(block_id) => node.on_block_commit(block_id),
        Update::PeerMessage(message, _id) => node.on_peer_message(message),
        Update::Shutdown => return false,

        update => warn!("Unhandled update: {:?}", update),
    }
    true
}
