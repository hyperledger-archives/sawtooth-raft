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

pub const PUBLISH_PERIOD: Duration = Duration::from_secs(3);

pub fn default_raft_config(id: u64) -> Config {
    let mut config = Config::default();
    config.id = id;
    config.heartbeat_tick = 150;
    config.election_tick = config.heartbeat_tick * 10;
    config.max_inflight_msgs = 10;
    config.validate().expect("Invalid Raft Config");
    config
}

pub fn storage() -> MemStorage {
    MemStorage::new()
}
