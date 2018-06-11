// Copyright 2018 PingCAP, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// See the License for the specific language governing permissions and
// limitations under the License.

extern crate raft;
extern crate sawtooth_sdk;

use std::process;

use raft::RawNode;
use sawtooth_sdk::consensus::zmq_driver::ZmqDriver;

mod config;
mod engine;
mod ticker;

// A simple example about how to use the Raft library in Rust.
fn main() {
    // Create a storage for Raft, and here we just use a simple memory storage.
    // You need to build your own persistent storage in your production.
    // Please check the Storage trait in src/storage.rs to see how to implement one.
    let storage = config::storage();

    // Create the configuration for the Raft node.
    let cfg = config::raft_config();

    // Create the Raft node.
    let node = RawNode::new(&cfg, storage, vec![]).unwrap();

    let raft_engine = engine::RaftEngine::new(node);

    let endpoint = "tcp://127.0.0.1:5050";

    let (driver, _stop) = ZmqDriver::new();
    driver.start(endpoint, raft_engine).unwrap_or_else(|err| {
        eprintln!("{}", err);
        process::exit(1);
    });
}
