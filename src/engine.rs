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
use std::collections::HashMap;
use std::time::Duration;
use std::sync::mpsc::{Receiver, RecvTimeoutError};

use raft::{
    Config as RaftConfig,
    raw_node::RawNode,
};
use serde_json;

use sawtooth_sdk::consensus::{
    engine::{Block, BlockId, PeerInfo, PeerId, Engine, Update},
    service::Service,
};

use config;
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

        // Create a storage for Raft, and here we just use a simple memory storage.
        // You need to build your own persistent storage in your production.
        // Please check the Storage trait in src/storage.rs to see how to implement one.
        let storage = config::storage();

        // Create the configuration for the Raft node.
        let (cfg, _peers) = self.load_raft_config(chain_head.block_id, &mut service);

        // Create the Raft node.
        let raw_node = RawNode::new(&cfg, storage, vec![]).unwrap();

        trace!("Initializing first block");
        service.initialize_block(None).expect("Initialize block failed");

        let mut node = SawtoothRaftNode::new(raw_node, service);

        let raft_timeout = RAFT_TIMEOUT;
        let publish_timeout = config::PUBLISH_PERIOD;

        let mut raft_ticker = ticker::Ticker::new(raft_timeout);
        let mut publish_ticker = ticker::Ticker::new(publish_timeout);

        let mut timeout = cmp::min(raft_timeout, publish_timeout);

        // Loop forever to drive the Raft.
        loop {
            trace!("Top of main loop");
            match updates.recv_timeout(timeout) {
                // Propose is the equivalent of publish block
                Ok(Update::BlockNew(block)) => node.on_block_new(block),
                Ok(Update::BlockValid(block_id)) => node.on_block_valid(block_id),
                Ok(Update::BlockCommit(block_id)) => node.on_block_commit(block_id),
                // This is a consensus message that should be passed to the node
                Ok(Update::PeerMessage(message, _id)) => node.on_peer_message(message),
                Err(RecvTimeoutError::Timeout) => (),
                Err(RecvTimeoutError::Disconnected) => return,
                _ => unimplemented!(),
            }

            let raft_timeout = raft_ticker.tick(|| {
                node.tick();
            });
            let publish_timeout = publish_ticker.tick(|| {
                node.propose_block()
            });
            timeout = cmp::min(raft_timeout, publish_timeout);

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

impl RaftEngine {
    fn load_raft_config(&self, block_id: BlockId, service: &mut Box<Service>) -> (RaftConfig, HashMap<PeerId, u64>) {
        let mut default_config = config::default_raft_config(self.id);

        let settings_keys = vec![
            "sawtooth.consensus.raft.peers",
            "sawtooth.consensus.raft.heartbeat_tick",
            "sawtooth.consensus.raft.election_tick",
        ];

        let settings: HashMap<String, String> = service
            .get_settings(block_id, settings_keys.into_iter().map(String::from).collect())
            .expect("Failed to get settings keys");

        if let Some(heartbeat_tick) = settings.get("sawtooth.consensus.raft.heartbeat_tick") {
            let parsed: Result<usize, _> = heartbeat_tick.parse();
            if let Ok(tick) = parsed {
                default_config.heartbeat_tick = tick;
            }
        }

        if let Some(election_tick) = settings.get("sawtooth.consensus.raft.election_tick") {
            let parsed: Result<usize, _> = election_tick.parse();
            if let Ok(tick) = parsed {
                default_config.election_tick = tick;
            }
        }

        let peers_setting_value = settings
            .get("sawtooth.consensus.raft.peers")
            .expect("'sawtooth.consensus.raft.peers' must be set to use Raft");

        let peers: HashMap<String, u64> = serde_json::from_str(peers_setting_value)
            .expect("Invalid value at 'sawtooth.consensus.raft.peers'");

        let peers: HashMap<PeerId, u64> = peers
            .into_iter()
            .map(|(s, id)| (PeerId::from(Vec::from(s)), id))
            .collect();

        let ids: Vec<u64> = peers.values().cloned().collect();
        default_config.peers = ids;

        (default_config, peers)
    }
}
