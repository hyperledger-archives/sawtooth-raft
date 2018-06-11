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
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;

use raft::{
    self,
    eraftpb::{
        EntryType,
        Message,
    },
    raw_node::RawNode,
    storage::MemStorage,
};

use sawtooth_sdk::consensus::{
    engine::{Block, PeerInfo, Engine, Update},
    service::Service,
};

use config;
use ticker;


type ProposeCallback = Box<Fn() + Send>;

enum Msg {
    Propose {
        id: u8,
        cb: ProposeCallback,
    },
    // Here we don't use Raft Message, so use dead_code to
    // avoid the compiler warning.
    #[allow(dead_code)]
    Raft(Message),
}

pub struct RaftEngine {
    raft_node: RawNode<MemStorage>,
}

impl RaftEngine {
    pub fn new(raft_node: RawNode<MemStorage>) -> Self {
        RaftEngine { raft_node }
    }
}

impl Engine for RaftEngine {
    fn start(
        &mut self,
        _updates: Receiver<Update>,
        _service: Box<Service>,
        _chain_head: Block,
        _peers: Vec<PeerInfo>,
    ) {
        let (sender, receiver) = mpsc::channel();

        // Loop forever to drive the Raft.
        let raft_timeout = config::RAFT_PERIOD;
        let publish_timeout = config::PUBLISH_PERIOD;

        let mut raft_ticker = ticker::Ticker::new(raft_timeout);
        let mut publish_ticker = ticker::Ticker::new(publish_timeout);

        let mut timeout = cmp::min(raft_timeout, publish_timeout);

        // Use a HashMap to hold the `propose` callbacks.
        let mut cbs = HashMap::new();

        loop {
            match receiver.recv_timeout(timeout) {
                // Propose is the equivalent of publish block
                Ok(Msg::Propose { id, cb }) => {
                    cbs.insert(id, cb);
                    self.raft_node.propose(vec![], vec![id]).unwrap();
                }
                // This is a consensus message that should be passed to the node
                Ok(Msg::Raft(m)) => self.raft_node.step(m).unwrap(),
                Err(RecvTimeoutError::Timeout) => (),
                Err(RecvTimeoutError::Disconnected) => return,
            }

            let raft_timeout = raft_ticker.tick(|| {
                self.raft_node.tick();
            });
            let publish_timeout = publish_ticker.tick(|| {
                match self.raft_node.raft.state {
                    raft::StateRole::Leader => {
                        // Use another thread to propose a Raft request.
                        send_propose(sender.clone());
                    },
                    _ => (),
                }
            });
            timeout = cmp::min(raft_timeout, publish_timeout);

            on_ready(&mut self.raft_node, &mut cbs);
        }
    }

    fn version(&self) -> String {
        "0.1".into()
    }

    fn name(&self) -> String {
        "Raft".into()
    }
}

fn on_ready(r: &mut RawNode<MemStorage>, cbs: &mut HashMap<u8, ProposeCallback>) {
    if !r.has_ready() {
        return;
    }

    // The Raft is ready, we can do something now.
    let mut ready = r.ready();

    let is_leader = r.raft.leader_id == 1;
    if is_leader {
        // If the peer is leader, the leader can send messages to other followers ASAP.
        let msgs = ready.messages.drain(..);
        for _msg in msgs {
            // Here we only have one peer, so can ignore this.
        }
    }

    if !raft::is_empty_snap(&ready.snapshot) {
        // This is a snapshot, we need to apply the snapshot at first.
        r.mut_store()
            .wl()
            .apply_snapshot(ready.snapshot.clone())
            .unwrap();
    }

    if !ready.entries.is_empty() {
        // Append entries to the Raft log
        r.mut_store().wl().append(&ready.entries).unwrap();
    }

    if let Some(ref hs) = ready.hs {
        // Raft HardState changed, and we need to persist it.
        r.mut_store().wl().set_hardstate(hs.clone());
    }

    if !is_leader {
        // If not leader, the follower needs to reply the messages to
        // the leader after appending Raft entries.
        let msgs = ready.messages.drain(..);
        for _msg in msgs {
            // Send messages to other peers.
        }
    }

    if let Some(committed_entries) = ready.committed_entries.take() {
        let mut _last_apply_index = 0;
        for entry in committed_entries {
            // Mostly, you need to save the last apply index to resume applying
            // after restart. Here we just ignore this because we use a Memory storage.
            _last_apply_index = entry.get_index();

            if entry.get_data().is_empty() {
                // Emtpy entry, when the peer becomes Leader it will send an empty entry.
                continue;
            }

            if entry.get_entry_type() == EntryType::EntryNormal {
                if let Some(cb) = cbs.remove(entry.get_data().get(0).unwrap()) {
                    cb();
                }
            }

            // TODO: handle EntryConfChange
        }
    }

    // Advance the Raft
    r.advance(ready);
}

fn send_propose(sender: mpsc::Sender<Msg>) {
    thread::spawn(move || {
        let (s1, r1) = mpsc::channel::<u8>();

        println!("propose a request");

        // Send a command to the Raft, wait for the Raft to apply it
        // and get the result.
        sender
            .send(Msg::Propose {
                id: 1,
                cb: Box::new(move || {
                    s1.send(0).unwrap();
                }),
            })
            .unwrap();

        let n = r1.recv().unwrap();
        assert_eq!(n, 0);

        println!("receive the propose callback");
    });
}
