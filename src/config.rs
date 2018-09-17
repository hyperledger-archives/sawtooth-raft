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
use std::fmt;
use std::time::Duration;

use hex;
use raft::Config as RaftConfig;
use sawtooth_sdk::consensus::{engine::{BlockId, PeerId}, service::Service};
use serde_json;

use cached_storage::CachedStorage;
use fs_storage::FsStorage;
use path::get_path_config;
use storage::StorageExt;

pub struct RaftEngineConfig<S: StorageExt> {
    pub peers: Vec<PeerId>,
    pub period: Duration,
    pub raft: RaftConfig,
    pub storage: S,
}

impl<S: StorageExt> RaftEngineConfig<S> {
    fn new(storage: S) -> Self {
        let mut raft = RaftConfig::default();
        raft.max_size_per_msg = 1024 * 1024 * 1024;

        RaftEngineConfig {
            peers: Vec::new(),
            period: Duration::from_millis(3_000),
            raft,
            storage,
        }
    }
}

fn create_storage() -> CachedStorage<impl StorageExt> {
    CachedStorage::new(
        FsStorage::with_data_dir(get_path_config().data_dir).expect("Failed to create FsStorage")
    )
}

impl<S: StorageExt> fmt::Debug for RaftEngineConfig<S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "RaftEngineConfig {{ peers: {:?}, period: {:?}, raft: {{ election_tick: {}, heartbeat_tick: {} }}, storage: {} }}",
            self.peers,
            self.period,
            self.raft.election_tick,
            self.raft.heartbeat_tick,
            S::describe(),
        )
    }
}

pub fn load_raft_config(
    peer_id: &PeerId,
    block_id: BlockId,
    service: &mut Box<Service>,
) -> RaftEngineConfig<impl StorageExt> {

    let mut config = RaftEngineConfig::new(create_storage());
    config.raft.id = peer_id_to_raft_id(peer_id);
    config.raft.tag = format!("[{}]", config.raft.id);

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

    let peers = get_peers_from_settings(&settings);

    let ids: Vec<u64> = peers.iter().map(peer_id_to_raft_id).collect();

    config.peers = peers;
    config.raft.peers = ids;

    config
}

/// Create a u64 value from the last eight bytes in the peer id
pub fn peer_id_to_raft_id(peer_id: &PeerId) -> u64 {
    let bytes: &[u8] = peer_id.as_ref();
    assert!(bytes.len() >= 8);
    let mut u: u64 = 0;
    for i in 0..8 {
        u += (u64::from(bytes[bytes.len() - 1 - i])) << (i * 8)
    }
    u
}

/// Get the peers as a Vec<PeerId> from settings
pub fn get_peers_from_settings(settings: &HashMap<String, String>) -> Vec<PeerId> {
    let peers_setting_value = settings
        .get("sawtooth.consensus.raft.peers")
        .expect("'sawtooth.consensus.raft.peers' must be set to use Raft");

    let peers: Vec<String> = serde_json::from_str(peers_setting_value)
        .expect("Invalid value at 'sawtooth.consensus.raft.peers'");

    peers.into_iter()
        .map(|s| PeerId::from(hex::decode(s).expect("Peer id not valid hex")))
        .collect()
}
