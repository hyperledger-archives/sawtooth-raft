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

use std::collections::VecDeque;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use raft::{
    self, eraftpb::{ConfState, Entry, HardState, Snapshot}, RaftState, Storage,
};

use storage::StorageExt;

pub const CACHE_SIZE: u64 = 10000;

struct Cache {
    entries: VecDeque<Entry>,
    _cache_size: u64,
}

impl Cache {
    fn with_cache_size(cache_size: u64) -> Cache {
        Cache {
            entries: VecDeque::new(),
            _cache_size: cache_size,
        }
    }

    fn first_index(&self) -> u64 {
        match self.entries.front() {
            Some(entry) => entry.index,
            None => 1,
        }
    }

    fn last_index(&self) -> u64 {
        match self.entries.back() {
            Some(entry) => entry.index,
            None => 0,
        }
    }

    fn entries(&self, low: u64, high: u64, _max_size: u64) -> Result<Vec<Entry>, raft::Error> {
        if self.entries.is_empty() || low < self.first_index() {
            return Err(raft::Error::Store(raft::StorageError::Compacted));
        }

        if high > self.last_index() + 1 {
            return Err(raft::Error::Store(raft::StorageError::Unavailable));
        }

        Ok(self.entries
            .iter()
            .cloned()
            .filter(|entry| entry.index >= low && entry.index < high)
            .collect())
    }

    fn term(&self, idx: u64) -> Option<u64> {
        self.entries
            .iter()
            .find(|ref entry| entry.index == idx)
            .map(|entry| entry.term)
    }

    fn compact(&mut self, compact_index: u64) {
        self.entries.retain(|entry| entry.index >= compact_index);
    }

    fn append(&mut self, entries: &[Entry]) {
        entries
            .into_iter()
            .for_each(|entry| self.entries.push_back(entry.clone()));
    }
}

pub struct CachedStorage<S: StorageExt> {
    storage: S,
    cache: Arc<RwLock<Cache>>,
}

impl<S: StorageExt> CachedStorage<S> {
    pub fn new(storage: S) -> Self {
        CachedStorage {
            storage,
            cache: Arc::new(RwLock::new(Cache::with_cache_size(CACHE_SIZE))),
        }
    }

    fn cache_read(&self) -> RwLockReadGuard<Cache> {
        self.cache.read().unwrap()
    }

    fn cache_write(&self) -> RwLockWriteGuard<Cache> {
        self.cache.write().unwrap()
    }
}

impl<S: StorageExt> Storage for CachedStorage<S> {
    fn initial_state(&self) -> Result<RaftState, raft::Error> {
        self.storage.initial_state()
    }

    fn entries(&self, low: u64, high: u64, _max_size: u64) -> Result<Vec<Entry>, raft::Error> {
        self.cache_read().entries(low, high, _max_size)
    }

    fn term(&self, idx: u64) -> Result<u64, raft::Error> {
        match self.cache_read().term(idx) {
            Some(term) => Ok(term),
            None => self.storage.term(idx),
        }
    }

    fn first_index(&self) -> Result<u64, raft::Error> {
        Ok(self.cache_read().first_index())
    }

    fn last_index(&self) -> Result<u64, raft::Error> {
        Ok(self.cache_read().last_index())
    }

    fn snapshot(&self) -> Result<Snapshot, raft::Error> {
        self.storage.snapshot()
    }
}

impl<S: StorageExt> StorageExt for CachedStorage<S> {
    fn set_hardstate(&self, hard_state: &HardState) {
        self.storage.set_hardstate(hard_state);
    }

    fn create_snapshot(
        &self,
        index: u64,
        conf_state: Option<&ConfState>,
        data: Vec<u8>,
    ) -> Result<Snapshot, raft::Error> {
        self.storage.create_snapshot(index, conf_state, data)
    }

    fn apply_snapshot(&self, snapshot: &Snapshot) -> Result<(), raft::Error> {
        self.storage.apply_snapshot(snapshot).map(|_| {
            let compact_index = snapshot.get_metadata().get_index();
            self.cache_write().compact(compact_index);
        })
    }

    fn compact(&self, compact_index: u64) -> Result<(), raft::Error> {
        self.storage.compact(compact_index).map(|_| {
            self.cache_write().compact(compact_index);
        })
    }

    fn append(&self, entries: &[Entry]) -> Result<(), raft::Error> {
        self.storage.append(entries).map(|_| {
            self.cache_write().append(entries);
        })
    }

    fn describe() -> String {
        format!("cached storage: {}", S::describe())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use tempdir::TempDir;

    use fs_storage::FsStorage;

    use raft::storage::MemStorage;

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

    fn create_temp_storage(name: &str) -> (TempDir, CachedStorage<FsStorage>) {
        let tmp = TempDir::new(name).unwrap();
        let storage = CachedStorage::new(
            FsStorage::with_data_dir(tmp.path().into()).expect("Failed to create FsStorage")
        );
        (tmp, storage)
    }

    fn populate_storage<S: StorageExt>(storage: &S, ids: Vec<u64>) -> Vec<Entry> {
        let entries = create_entries(ids);
        storage.append(&entries).unwrap();
        entries
    }

    // Storage trait tests

    // Test that state is initialize when CachedStorage is created
    #[test]
    fn test_storage_initial_state() {
        let (_tmp, storage) = create_temp_storage("test_storage_initial_state");

        let RaftState {
            hard_state,
            conf_state,
        } = storage.initial_state().unwrap();

        assert_eq!(HardState::default(), hard_state);
        assert_eq!(ConfState::default(), conf_state);
    }

    #[test]
    fn test_storage_entries() {
        let (_tmp, storage) = create_temp_storage("test_storage_entries");

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

    #[test]
    fn test_storage_term() {
        let (_tmp, storage) = create_temp_storage("test_storage_term");

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

    #[test]
    fn test_first_and_last_index() {
        let (_tmp, storage) = create_temp_storage("test_first_and_last_index");

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

    #[test]
    fn test_storage_ext_compact() {
        let (_tmp, storage) = create_temp_storage("test_storage_ext_compact");

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

    // Test that both implementations of StorageExt produce the same results
    #[test]
    fn test_parity() {
        let (_tmp, cached_storage) = create_temp_storage("test_parity");

        let mem_storage = MemStorage::new();

        assert_eq!(mem_storage.term(0), cached_storage.term(0));
        assert_eq!(mem_storage.last_index(), cached_storage.last_index());
        assert_eq!(mem_storage.first_index(), cached_storage.first_index());

        for i in 0..3 {
            assert_eq!(
                mem_storage.entries(0, i, MAX),
                cached_storage.entries(0, i, MAX)
            );
        }

        populate_storage(&mem_storage, (1..6).collect());
        populate_storage(&cached_storage, (1..6).collect());

        assert_eq!(mem_storage.first_index(), cached_storage.first_index());
        assert_eq!(mem_storage.last_index(), cached_storage.last_index());

        // NOTE: This starts at i=1 because I believe the implementation of MemStorage does the
        // wrong thing for (0, 0)
        for i in 1..6 {
            for j in i..6 {
                assert_eq!(
                    mem_storage.entries(i, j, MAX),
                    cached_storage.entries(i, j, MAX)
                );
            }
            assert_eq!(mem_storage.term(i), cached_storage.term(i));
        }

        assert_eq!(mem_storage.snapshot(), cached_storage.snapshot());

        let mem_snapshot = mem_storage
            .create_snapshot(3, None, "".into())
            .expect("MemStorage: Create snapshot failed");
        let fs_snapshot = cached_storage
            .create_snapshot(3, None, "".into())
            .expect("CachedStorage: Create snapshot failed");
        assert_eq!(mem_snapshot, fs_snapshot);

        assert_eq!(
            mem_storage.apply_snapshot(&mem_snapshot),
            cached_storage.apply_snapshot(&fs_snapshot),
        );

        for i in 2..5 {
            assert_eq!(mem_storage.compact(i), cached_storage.compact(i));

            assert_eq!(mem_storage.first_index(), cached_storage.first_index());
            assert_eq!(mem_storage.last_index(), cached_storage.last_index());
        }

        assert_eq!(mem_storage.snapshot(), cached_storage.snapshot());
    }
}
