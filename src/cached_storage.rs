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

use std::cell::RefCell;
use std::ops::Range;

use raft::{
    self,
    eraftpb::{ConfState, Entry, HardState, Snapshot},
    RaftState, Storage,
};
use uluru;

use storage::StorageExt;

const CACHE_SIZE: usize = 16;

#[derive(Default)]
struct EntryCache(uluru::LRUCache<[uluru::Entry<Entry>; CACHE_SIZE]>);

impl EntryCache {
    fn get(&mut self, index: u64) -> Option<Entry> {
        self.0.find(|entry| entry.index == index).cloned()
    }

    fn insert(&mut self, entry: Entry) {
        self.0.insert(entry)
    }

    fn clear(&mut self) {
        self.0.evict_all()
    }

    fn range(&mut self, range: Range<u64>) -> Option<Vec<Entry>> {
        let mut entries: Vec<Entry> = Vec::new();
        for index in range {
            if let Some(entry) = self.get(index) {
                entries.push(entry);
            } else {
                return None;
            }
        }
        Some(entries)
    }
}

#[derive(Default)]
struct TermCache(uluru::LRUCache<[uluru::Entry<(u64, u64)>; CACHE_SIZE]>);

impl TermCache {
    fn get(&mut self, idx: u64) -> Option<u64> {
        self.0
            .find(|(index, _term)| *index == idx)
            .map(|(_index, term)| *term)
    }

    fn insert(&mut self, index: u64, term: u64) {
        self.0.insert((index, term))
    }

    fn clear(&mut self) {
        self.0.evict_all()
    }
}

struct StorageCache {
    pub initial_state: Option<RaftState>,
    pub first_index: Option<u64>,
    pub last_index: Option<u64>,
    pub snapshot: Option<Snapshot>,
    // We use Option<_> here because we always need to call down to the underlying storage after
    // reset, even if the call can be fulfilled without doing so, to handle startup edge cases.
    // For example, a call with an empty but valid range would return an empty Vec<_> if not for
    // this Option<_>, even though the underlying storage might return Err(Unavailable) because of
    // an empty log.
    pub entries: Option<EntryCache>,
    pub terms: TermCache,
}

impl Default for StorageCache {
    fn default() -> Self {
        StorageCache {
            initial_state: None,
            first_index: None,
            last_index: None,
            snapshot: None,
            entries: None,
            terms: Default::default(),
        }
    }
}

impl StorageCache {
    fn reset(&mut self) {
        self.initial_state = None;
        self.first_index = None;
        self.last_index = None;
        self.snapshot = None;
        if let Some(ref mut entries) = self.entries {
            entries.clear();
        }
        self.terms.clear();
    }
}

pub struct CachedStorage<S: StorageExt> {
    storage: S,
    cache: RefCell<StorageCache>,
}

impl<S: StorageExt> CachedStorage<S> {
    pub fn new(storage: S) -> Self {
        CachedStorage {
            storage,
            cache: Default::default(),
        }
    }
}

impl<S: StorageExt> Storage for CachedStorage<S> {
    fn initial_state(&self) -> Result<RaftState, raft::Error> {
        let mut cache = self.cache.borrow_mut();
        if let Some(ref initial_state) = cache.initial_state {
            return Ok(initial_state.clone());
        }

        self.storage.initial_state().map(|initial_state| {
            cache.initial_state = Some(initial_state.clone());
            initial_state
        })
    }

    fn entries(&self, low: u64, high: u64, max_size: u64) -> Result<Vec<Entry>, raft::Error> {
        let mut cache = self.cache.borrow_mut();

        if let Some(ref mut entries) = cache.entries {
            return match entries.range(low..high) {
                Some(range) => {
                    for entry in range.iter().cloned() {
                        entries.insert(entry);
                    }
                    Ok(range)
                }
                None => self.storage.entries(low, high, max_size),
            };
        }

        match self.storage.entries(low, high, max_size) {
            Ok(range) => {
                let mut entries = EntryCache::default();
                for entry in range.iter().cloned() {
                    entries.insert(entry);
                }
                cache.entries = Some(entries);
                Ok(range)
            }
            Err(err) => Err(err),
        }
    }

    fn term(&self, idx: u64) -> Result<u64, raft::Error> {
        let mut cache = self.cache.borrow_mut();
        if let Some(term) = cache.terms.get(idx) {
            Ok(term)
        } else {
            self.storage.term(idx).map(|term| {
                cache.terms.insert(idx, term);
                term
            })
        }
    }

    fn first_index(&self) -> Result<u64, raft::Error> {
        let mut cache = self.cache.borrow_mut();
        if let Some(ref first_index) = cache.first_index {
            return Ok(*first_index);
        }

        self.storage.first_index().map(|first_index| {
            cache.first_index = Some(first_index);
            first_index
        })
    }

    fn last_index(&self) -> Result<u64, raft::Error> {
        let mut cache = self.cache.borrow_mut();
        if let Some(ref last_index) = cache.last_index {
            return Ok(*last_index);
        }

        self.storage.last_index().map(|last_index| {
            cache.last_index = Some(last_index);
            last_index
        })
    }

    fn snapshot(&self) -> Result<Snapshot, raft::Error> {
        let mut cache = self.cache.borrow_mut();
        if let Some(ref snapshot) = cache.snapshot {
            return Ok(snapshot.clone());
        }

        self.storage.snapshot().map(|snapshot| {
            cache.snapshot = Some(snapshot.clone());
            snapshot
        })
    }
}

impl<S: StorageExt> StorageExt for CachedStorage<S> {
    fn set_hardstate(&self, hard_state: &HardState) {
        self.cache.borrow_mut().reset();
        self.storage.set_hardstate(hard_state)
    }

    fn create_snapshot(
        &self,
        index: u64,
        conf_state: Option<&ConfState>,
        data: Vec<u8>,
    ) -> Result<Snapshot, raft::Error> {
        self.cache.borrow_mut().reset();
        self.storage.create_snapshot(index, conf_state, data)
    }

    fn apply_snapshot(&self, snapshot: &Snapshot) -> Result<(), raft::Error> {
        self.cache.borrow_mut().reset();
        self.storage.apply_snapshot(snapshot)
    }

    fn compact(&self, compact_index: u64) -> Result<(), raft::Error> {
        self.cache.borrow_mut().reset();
        self.storage.compact(compact_index)
    }

    fn append(&self, entries: &[Entry]) -> Result<(), raft::Error> {
        self.cache.borrow_mut().reset();
        self.storage.append(entries)
    }

    // This is only read once (on startup), so it does not need to be cached
    fn applied(&self) -> Result<u64, raft::Error> {
        self.storage.applied()
    }

    fn set_applied(&self, applied: u64) -> Result<(), raft::Error> {
        self.storage.set_applied(applied)
    }

    fn describe() -> String {
        format!("cached storage: {}", S::describe())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use tempfile::Builder;
    use tempfile::TempDir;

    use fs_storage::FsStorage;
    use storage::tests;

    fn create_temp_storage(name: &str) -> (TempDir, CachedStorage<FsStorage>) {
        let tmp = Builder::new().prefix(name).tempdir().unwrap();
        let storage = CachedStorage::new(
            FsStorage::with_data_dir(tmp.path().into()).expect("Failed to create FsStorage"),
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
    fn test_applied() {
        let (_tmp, storage) = create_temp_storage("test_applied");
        tests::test_applied(storage);
    }

    #[test]
    fn test_parity() {
        let (_tmp, storage) = create_temp_storage("test_parity");
        tests::test_parity(storage);
    }
}
