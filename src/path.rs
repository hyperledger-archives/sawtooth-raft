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

use std::env;
use std::path::{Path, PathBuf};

const DEFAULT_DATA_DIR: &str = "/var/lib/sawtooth-raft";

pub struct PathConfig {
    pub data_dir: PathBuf,
}

pub fn get_path_config() -> PathConfig {
    match env::var("SAWTOOTH_RAFT_HOME") {
        Ok(prefix) => PathConfig {
            data_dir: Path::new(&prefix).join("data").to_path_buf(),
        },
        Err(_) => PathConfig {
            data_dir: Path::new(DEFAULT_DATA_DIR).to_path_buf(),
        },
    }
}
