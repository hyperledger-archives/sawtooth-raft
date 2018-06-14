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

use std::collections::HashMap;
use std::time::Duration;

use hex;
use raft::{
    Config as RaftConfig,
    storage::{MemStorage},
};
use sawtooth_sdk::consensus::{engine::{BlockId, PeerId}, service::Service};
use serde_json;

pub struct RaftEngineConfig {
    pub peers: HashMap<PeerId, u64>,
    // TODO: Find a better name for this
    pub period: Duration,
    pub raft: RaftConfig,
    pub storage: MemStorage,
}

impl Default for RaftEngineConfig {
    fn default() -> Self {
        let mut raft = RaftConfig::default();
        raft.election_tick = 10;
        raft.heartbeat_tick = 3;
        raft.max_inflight_msgs = 256;

        // TODO: Implement persistent storage
        RaftEngineConfig {
            peers: HashMap::new(),
            period: Duration::from_millis(3_000),
            raft,
            storage: MemStorage::new(),
        }
    }
}

pub fn load_raft_config(
    raft_id: u64,
    block_id: BlockId,
    service: &mut Box<Service>,
) -> RaftEngineConfig {

    let mut config = RaftEngineConfig::default();
    config.raft.id = raft_id;

    let settings_keys = vec![
        "sawtooth.consensus.raft.peers",
        "sawtooth.consensus.raft.heartbeat_tick",
        "sawtooth.consensus.raft.election_tick",
        "sawtooth.consensus.raft.period",
    ];

    let settings: HashMap<String, String> = service
        .get_settings(block_id, settings_keys.into_iter().map(String::from).collect())
        .expect("Failed to get settings keys");

    if let Some(heartbeat_tick) = settings.get("sawtooth.consensus.raft.heartbeat_tick") {
        let parsed: Result<usize, _> = heartbeat_tick.parse();
        if let Ok(tick) = parsed {
            config.raft.heartbeat_tick = tick;
        }
    }

    if let Some(election_tick) = settings.get("sawtooth.consensus.raft.election_tick") {
        let parsed: Result<usize, _> = election_tick.parse();
        if let Ok(tick) = parsed {
            config.raft.election_tick = tick;
        }
    }

    if let Some(period) = settings.get("sawtooth.consensus.raft.period") {
        let parsed: Result<u64, _> = period.parse();
        if let Ok(period) = parsed {
            config.period = Duration::from_millis(period);
        }
    }

    let peers_setting_value = settings
        .get("sawtooth.consensus.raft.peers")
        .expect("'sawtooth.consensus.raft.peers' must be set to use Raft");

    let peers: HashMap<String, u64> = serde_json::from_str(peers_setting_value)
        .expect("Invalid value at 'sawtooth.consensus.raft.peers'");

    let peers: HashMap<PeerId, u64> = peers
        .into_iter()
        .map(|(s, id)| (PeerId::from(hex::decode(s).expect("Peer id not valid hex")), id))
        .collect();

    let ids: Vec<u64> = peers.values().cloned().collect();

    config.peers = peers;
    config.raft.peers = ids;

    config
}
