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

#[macro_use]
extern crate clap;
extern crate hex;
#[macro_use]
extern crate log;
extern crate protobuf;
extern crate raft;
extern crate sawtooth_sdk;
extern crate simple_logger;
extern crate serde_json;

use std::process;

use sawtooth_sdk::consensus::zmq_driver::ZmqDriver;

mod config;
mod engine;
mod node;
mod ticker;

fn main() {
    let args = parse_args();

    simple_logger::init_with_level(args.log_level).unwrap();

    info!("Sawtooth Raft Engine ({})", env!("CARGO_PKG_VERSION"));

    let raft_engine = engine::RaftEngine::new(args.id);

    let (driver, _stop) = ZmqDriver::new();

    info!("Raft Node {} connecting to '{}'", &args.id, &args.endpoint);
    driver.start(&args.endpoint, raft_engine).unwrap_or_else(|err| {
        error!("{}", err);
        process::exit(1);
    });
}

fn parse_args() -> RaftCliArgs {
    let matches = clap_app!(sawtooth_raft =>
        (version: crate_version!())
        (about: "Raft consensus for Sawtooth")
        (@arg connect: -C --connect +takes_value
         "connection endpoint for validator")
        (@arg verbose: -v --verbose +multiple
         "increase output verbosity")
        (@arg ID: +required "the raft node's id"))
        .get_matches();

    let log_level = match matches.occurrences_of("verbose") {
        0 =>  log::Level::Warn,
        1 =>  log::Level::Info,
        2 =>  log::Level::Debug,
        3 | _ =>  log::Level::Trace,
    };

    let endpoint = matches
        .value_of("connect")
        .unwrap_or("tcp://localhost:5050")
        .into();

    let id = value_t!(matches.value_of("ID"), u64)
        .unwrap_or_else(|e| e.exit());

    RaftCliArgs {
        log_level,
        endpoint,
        id,
    }
}

pub struct RaftCliArgs {
    log_level: log::Level,
    endpoint: String,
    id: u64,
}
