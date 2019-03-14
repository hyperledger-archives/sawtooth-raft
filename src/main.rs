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
extern crate log4rs;
extern crate log4rs_syslog;
extern crate protobuf;
extern crate raft;
extern crate sawtooth_sdk;
extern crate serde_json;
extern crate uluru;

#[cfg(test)]
extern crate tempfile;

use log4rs::append::console::ConsoleAppender;
use log4rs::config::{Appender, Config, Root};
use log4rs::encode::pattern::PatternEncoder;

use std::process;

use sawtooth_sdk::consensus::zmq_driver::ZmqDriver;

mod block_queue;
mod cached_storage;
mod config;
mod engine;
mod fs_storage;
mod node;
mod path;
mod storage;
mod ticker;

fn main() {
    let args = parse_args();

    let config = match args.log_config {
        Some(path) => {
            // Register deserializer for syslog so we can load syslog appender(s)
            let mut deserializers = log4rs::file::Deserializers::new();
            log4rs_syslog::register(&mut deserializers);

            match log4rs::load_config_file(path, deserializers) {
                Ok(mut config) => {
                    {
                        let mut root = config.root_mut();
                        root.set_level(args.log_level);
                    }
                    config
                }
                Err(err) => {
                    eprintln!(
                        "Error loading logging configuration file: {:?}\
                         \nFalling back to console logging.",
                        err
                    );
                    get_console_config(args.log_level)
                }
            }
        }
        None => get_console_config(args.log_level),
    };

    log4rs::init_config(config).unwrap_or_else(|err| {
        eprintln!("Error initializing logging configuration: {:?}", err);
        process::exit(1)
    });

    info!("Sawtooth Raft Engine ({})", env!("CARGO_PKG_VERSION"));

    let raft_engine = engine::RaftEngine::new();

    let (driver, _stop) = ZmqDriver::new();

    info!("Raft Node connecting to '{}'", &args.endpoint);
    driver.start(&args.endpoint, raft_engine).unwrap_or_else(|err| {
        error!("{}", err);
        process::exit(1);
    });
}

fn get_console_config(log_level: log::LevelFilter) -> Config {
    let stdout = ConsoleAppender::builder()
        .encoder(Box::new(PatternEncoder::new(
            "{h({l:5.5})} | {({M}:{L}):20.20} | {m}{n}",
        )))
        .build();

    Config::builder()
        .appender(Appender::builder().build("stdout", Box::new(stdout)))
        .build(Root::builder().appender("stdout").build(log_level))
        .unwrap_or_else(|err| {
            eprintln!("Error building logging configuration: {:?}", err);
            process::exit(1)
        })
}

fn parse_args() -> RaftCliArgs {
    let matches = clap_app!(sawtooth_raft =>
        (version: crate_version!())
        (about: "Raft consensus for Sawtooth")
        (@arg connect: -C --connect +takes_value
         "connection endpoint for validator")
        (@arg verbose: -v --verbose +multiple
         "increase output verbosity")
        (@arg logconfig: -L --log_config +takes_value
         "path to logging config file"))
        .get_matches();

    let log_level = match matches.occurrences_of("verbose") {
        0 => log::LevelFilter::Warn,
        1 => log::LevelFilter::Info,
        2 | _ => log::LevelFilter::Trace,
    };

    let log_config = matches.value_of("logconfig").map(|s| s.into());

    let endpoint = matches
        .value_of("connect")
        .unwrap_or("tcp://localhost:5050")
        .into();

    RaftCliArgs {
        log_config,
        log_level,
        endpoint,
    }
}

pub struct RaftCliArgs {
    log_config: Option<String>,
    log_level: log::LevelFilter,
    endpoint: String,
}
