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

use raft::{Error, eraftpb::{Entry, HardState, Snapshot}, storage::{MemStorage, Storage}};

/// Extends the storage trait to include methods used by SawtoothRaftNode and provided by the
/// MemStorage type.
pub trait StorageExt: Storage {
    /// set_hardstate saves the current HardState.
    fn set_hardstate(&self, hs: HardState);

    /// apply_snapshot overwrites the contents of this Storage object with those of the given
    /// snapshot.
    fn apply_snapshot(&self, snapshot: Snapshot) -> Result<(), Error>;

    /// compact discards all log entries prior to compact_index. It is the application's
    /// responsibility to not attempt to compact an index greater than RaftLog.applied.
    fn compact(&self, compact_index: u64) -> Result<(), Error>;

    /// Append the new entries to storage
    fn append(&self, ents: &[Entry]) -> Result<(), Error>;
}

impl StorageExt for MemStorage {
    fn set_hardstate(&self, hs: HardState) {
        self.wl().set_hardstate(hs)
    }

    fn apply_snapshot(&self, snapshot: Snapshot) -> Result<(), Error> {
        self.wl().apply_snapshot(snapshot)
    }

    fn compact(&self, compact_index: u64) -> Result<(), Error> {
        self.wl().compact(compact_index)
    }

    fn append(&self, ents: &[Entry]) -> Result<(), Error> {
        self.wl().append(ents)
    }
}
