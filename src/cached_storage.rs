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
        self.entries.retain(|entry| entry.index > compact_index);
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
    use storage::tests;

    fn create_temp_storage(name: &str) -> (TempDir, CachedStorage<FsStorage>) {
        let tmp = TempDir::new(name).unwrap();
        let storage = CachedStorage::new(
            FsStorage::with_data_dir(tmp.path().into()).expect("Failed to create FsStorage")
        );
        (tmp, storage)
    }

    #[test]
    fn test_storage_initial_state() {
        let (_tmp, storage) = create_temp_storage("test_storage_initial_state");
        tests::test_storage_initial_state(storage);
    }

    #[test]
    fn test_storage_entries() {
        let (_tmp, storage) = create_temp_storage("test_storage_entries");
        tests::test_storage_entries(storage);
    }

    #[test]
    fn test_storage_term() {
        let (_tmp, storage) = create_temp_storage("test_storage_term");
        tests::test_storage_term(storage);
    }

    #[test]
    fn test_first_and_last_index() {
        let (_tmp, storage) = create_temp_storage("test_first_and_last_index");
        tests::test_first_and_last_index(storage);
    }


    #[test]
    fn test_storage_ext_compact() {
        let (_tmp, storage) = create_temp_storage("test_storage_ext_compact");
        tests::test_storage_ext_compact(storage);
    }

    #[test]
    fn test_parity() {
        let (_tmp, storage) = create_temp_storage("test_parity");
        tests::test_parity(storage);
    }
}
