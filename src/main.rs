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

#[macro_use]
extern crate clap;
#[macro_use]
extern crate log;
extern crate protobuf;
extern crate raft;
extern crate sawtooth_sdk;
extern crate simple_logger;

use std::process;

use sawtooth_sdk::consensus::zmq_driver::ZmqDriver;

mod config;
mod engine;
mod node;
mod ticker;

// A simple example about how to use the Raft library in Rust.
fn main() {
    let engine_config = config::engine_config();
    simple_logger::init().unwrap();

    info!("{}", &engine_config.about);

    let raft_engine = engine::RaftEngine::new(engine_config.id);

    let (driver, _stop) = ZmqDriver::new();

    info!("Connecting to '{}'", &engine_config.endpoint);
    driver.start(&engine_config.endpoint, raft_engine).unwrap_or_else(|err| {
        error!("{}", err);
        process::exit(1);
    });
}
