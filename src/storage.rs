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

use raft::{Error, eraftpb::{ConfState, Entry, HardState, Snapshot}, storage::{MemStorage, Storage}};

/// Extends the storage trait to include methods used by SawtoothRaftNode and provided by the
/// MemStorage type.
pub trait StorageExt: Storage {
    /// set_hardstate saves the current HardState.
    fn set_hardstate(&self, hard_state: &HardState);

    /// apply_snapshot overwrites the contents of this Storage object with those of the given
    /// snapshot.
    fn apply_snapshot(&self, snapshot: &Snapshot) -> Result<(), Error>;

    /// creates and applies a new snapshot, returning a clone of the created snapshot
    fn create_snapshot(
        &self,
        index: u64,
        conf_state: Option<&ConfState>,
        data: Vec<u8>,
    ) -> Result<Snapshot, Error>;

    /// compact discards all log entries prior to compact_index. It is the application's
    /// responsibility to not attempt to compact an index greater than RaftLog.applied.
    fn compact(&self, compact_index: u64) -> Result<(), Error>;

    /// Append the new entries to storage
    fn append(&self, ents: &[Entry]) -> Result<(), Error>;

    /// Get the index of the last raft entry that was committed
    fn applied(&self) -> Result<u64, Error>;

    /// Save the index of the last raft entry that was committed
    fn set_applied(&self, applied: u64) -> Result<(), Error>;

    fn describe() -> String;
}

impl StorageExt for MemStorage {
    fn set_hardstate(&self, hs: &HardState) {
        self.wl().set_hardstate(hs.clone())
    }

    fn create_snapshot(
        &self,
        idx: u64,
        cs: Option<&ConfState>,
        data: Vec<u8>,
    ) -> Result<Snapshot, Error> {
        self.wl().create_snapshot(idx, cs.map(ConfState::clone), data).map(Snapshot::clone)
    }


    fn apply_snapshot(&self, snapshot: &Snapshot) -> Result<(), Error> {
        self.wl().apply_snapshot(snapshot.clone())
    }

    fn compact(&self, compact_index: u64) -> Result<(), Error> {
        self.wl().compact(compact_index)
    }

    fn append(&self, ents: &[Entry]) -> Result<(), Error> {
        self.wl().append(ents)
    }

    // Applied index is only useful when node restarts, but MemStorage does not persist
    fn applied(&self) -> Result<u64, Error> {
        Ok(0)
    }

    fn set_applied(&self, _applied: u64) -> Result<(), Error> {
        Ok(())
    }

    fn describe() -> String {
        "in-memory storage".into()
    }
}

#[cfg(test)]
pub (crate) mod tests {
    use super::*;

    use raft::{
        self, storage::MemStorage, eraftpb::{ConfState, Entry, HardState}, RaftState, Storage,
    };

    const MAX: u64 = u64::max_value();

    fn create_entry(index: u64, term: u64) -> Entry {
        let mut e = Entry::new();
        e.set_term(term);
        e.set_index(index);
        e
    }

    fn create_entries(ids: Vec<u64>) -> Vec<Entry> {
        ids.into_iter().map(|i| create_entry(i, i)).collect()
    }

    fn populate_storage<S: StorageExt>(storage: &S, ids: Vec<u64>) -> Vec<Entry> {
        let entries = create_entries(ids);
        storage.append(&entries).unwrap();
        entries
    }


    // Storage trait tests

    // Test that state is initialize when FsStorage is created
    pub fn test_storage_initial_state<S: StorageExt>(storage: S) {
        let RaftState {
            hard_state,
            conf_state,
        } = storage.initial_state().unwrap();

        assert_eq!(HardState::default(), hard_state);
        assert_eq!(ConfState::default(), conf_state);
    }

    pub fn test_storage_entries<S: StorageExt>(storage: S) {
        // Assert that we get an error no matter what when there are no entries
        for i in 0..4 {
            for j in 0..4 {
                assert_eq!(
                    Err(raft::Error::Store(raft::StorageError::Compacted)),
                    storage.entries(i, j, MAX)
                );
            }
        }

        // Write entries 1-3
        let entries = populate_storage(&storage, (1..4).collect());

        // Verify we get the entries we wrote
        for i in 1..4 {
            for j in i..4 {
                assert_eq!(
                    Ok(entries[i-1..j-1].to_vec()),
                    storage.entries(i as u64, j as u64, MAX)
                );
            }
        }
    }

    pub fn test_storage_term<S: StorageExt>(storage: S) {
        // Test that even when there are no entries, we get a term for 0
        assert_eq!(Ok(0), storage.term(0));

        // Write some entries
        let entries = populate_storage(&storage, (1..6).collect());

        // Assert we get the right terms
        for entry in entries {
            assert_eq!(Ok(entry.term), storage.term(entry.index));
        }

        // Delete some entries
        storage.compact(3).unwrap();

        // Test that we can still get the term of the last compacted entry
        assert_eq!(
            Err(raft::Error::Store(raft::StorageError::Compacted)),
            storage.term(1)
        );
        assert_eq!(
            Err(raft::Error::Store(raft::StorageError::Compacted)),
            storage.term(2)
        );
        assert_eq!(Ok(3), storage.term(3));
        assert_eq!(Ok(4), storage.term(4));
    }

    pub fn test_first_and_last_index<S: StorageExt>(storage: S) {
        // Assert indexes when nothing available, weirdly, first_index() should return 1 in this
        // case, otherwise a less-than-0 overflow occurs when Raft is initialized.
        assert_eq!(Ok(1), storage.first_index());
        assert_eq!(Ok(0), storage.last_index());

        populate_storage(&storage, (1..6).collect());

        assert_eq!(Ok(1), storage.first_index());
        assert_eq!(Ok(5), storage.last_index());

        storage.compact(3).unwrap();

        assert_eq!(Ok(4), storage.first_index());
        assert_eq!(Ok(5), storage.last_index());
    }


    // StorageExt trait tests

    pub fn test_storage_ext_compact<S: StorageExt>(storage: S) {
        // Assert that compacting fails when there is nothing to compact
        assert_eq!(
            Err(raft::Error::Store(raft::StorageError::Compacted)),
            storage.compact(0)
        );

        let entries = populate_storage(&storage, (1..11).collect());

        assert_eq!(
            Err(raft::Error::Store(raft::StorageError::Compacted)),
            storage.compact(0)
        );

        assert_eq!(Ok(()), storage.compact(2));
        assert_eq!(
            Err(raft::Error::Store(raft::StorageError::Compacted)),
            storage.entries(1, 3, MAX)
        );
        assert_eq!(Ok(entries[2..9].to_vec()), storage.entries(3, 10, MAX));

        assert_eq!(Ok(()), storage.compact(4));
        assert_eq!(
            Err(raft::Error::Store(raft::StorageError::Compacted)),
            storage.entries(2, 5, MAX)
        );
        assert_eq!(Ok(entries[4..9].to_vec()), storage.entries(5, 10, MAX));
    }

    pub fn test_applied<S: StorageExt>(storage: S) {
        // Applied is initially 0
        assert_eq!(Ok(0), storage.applied());

        // Applied gets set
        storage.set_applied(1);
        assert_eq!(Ok(1), storage.applied());
        storage.set_applied(2);
        assert_eq!(Ok(2), storage.applied());
    }

    // Test that both implementations of StorageExt produce the same results
    pub fn test_parity<S: StorageExt>(storage: S) {
        let mem_storage = MemStorage::new();

        assert_eq!(mem_storage.term(0), storage.term(0));
        assert_eq!(mem_storage.term(1), storage.term(1));
        assert_eq!(mem_storage.term(2), storage.term(2));
        assert_eq!(mem_storage.last_index(), storage.last_index());
        assert_eq!(mem_storage.first_index(), storage.first_index());

        for i in 0..3 {
            assert_eq!(mem_storage.entries(0, i, MAX), storage.entries(0, i, MAX));
        }

        populate_storage(&mem_storage, (1..6).collect());
        populate_storage(&storage, (1..6).collect());

        assert_eq!(mem_storage.first_index(), storage.first_index());
        assert_eq!(mem_storage.last_index(), storage.last_index());

        // NOTE: This starts at i=1 because I believe the implementation of MemStorage does the
        // wrong thing for (0, 0)
        for i in 1..6 {
            for j in i..6 {
                assert_eq!(mem_storage.entries(i, j, MAX), storage.entries(i, j, MAX));
            }
            assert_eq!(mem_storage.term(i), storage.term(i));
        }

        assert_eq!(mem_storage.snapshot(), storage.snapshot());

        let mem_snapshot = mem_storage.create_snapshot(3, None, "".into()).expect("MemStorage: Create snapshot failed");
        let fs_snapshot = storage.create_snapshot(3, None, "".into()).expect("FsStorage: Create snapshot failed");
        assert_eq!(mem_snapshot, fs_snapshot);

        assert_eq!(
            mem_storage.apply_snapshot(&mem_snapshot),
            storage.apply_snapshot(&fs_snapshot),
        );

        for i in 2..5 {
            assert_eq!(mem_storage.compact(i), storage.compact(i));

            assert_eq!(mem_storage.first_index(), storage.first_index());
            assert_eq!(mem_storage.last_index(), storage.last_index());
        }

        assert_eq!(mem_storage.snapshot(), storage.snapshot());
    }
}
