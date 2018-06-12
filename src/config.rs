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

use raft::{Config, storage::{MemStorage}};

pub const RAFT_PERIOD: Duration = Duration::from_millis(100);
pub const PUBLISH_PERIOD: Duration = Duration::from_secs(3);

pub fn raft_config() -> Config {
    let mut config = Config::default();
    config.id = 1;
    config.heartbeat_tick = 150;
    config.election_tick = config.heartbeat_tick * 10;
    config.max_inflight_msgs = 10;
    config.validate().expect("Invalid Raft Config");
    config
}

pub fn storage() -> MemStorage {
    MemStorage::new()
}

pub struct RaftEngineConfig {
    pub about: String,
    pub endpoint: String,
}

pub fn engine_config() -> RaftEngineConfig {
    let about = format!("Sawtooth Raft Engine ({})", env!("CARGO_PKG_VERSION"));

    let matches = clap_app!(sawtooth_raft =>
        (version: crate_version!())
        (about: "Raft consensus for Sawtooth")
        (@arg connect: -C --connect +takes_value
         "connection endpoint for validator")
        (@arg verbose: -v --verbose +multiple
         "increase output verbosity"))
        .get_matches();

    let endpoint = matches
        .value_of("connect")
        .unwrap_or("tcp://localhost:5050");

    RaftEngineConfig {
        about: about.clone(),
        endpoint: endpoint.into(),
    }
}
