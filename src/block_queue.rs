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

use std::collections::{HashMap, VecDeque};

 use sawtooth_sdk::consensus::{
     engine::{Block, BlockId},
 };

struct BlockStatus {
    // Block received in BlockNew message
    block: Option<Block>,
    // Block has been validated
    block_valid: bool,
}

impl BlockStatus {
    fn with_block(block: Block) -> Self {
        BlockStatus {
            block: Some(block),
            block_valid: false,
        }
    }

    fn with_valid() -> Self {
        BlockStatus {
            block: None,
            block_valid: true,
        }
    }
}

pub struct BlockQueue {
    // Backlog of blocks that are being/have been processed by the validator
    validator_backlog: HashMap<BlockId, BlockStatus>,
    // Queue of blocks that can be committed when they have been validated, along with the index of
    // the corresponding Raft entry
    commit_queue: VecDeque<(BlockId, u64)>,
}

impl BlockQueue {
    pub fn new() -> Self {
        BlockQueue {
            validator_backlog: HashMap::new(),
            commit_queue: VecDeque::new(),
        }
    }

    // Save the block; called when the BlockNew message is received by the engine
    pub fn block_new(&mut self, block: Block) {
        let block_id = block.block_id.clone();
        self.validator_backlog.entry(block_id).and_modify(|status| {
            status.block = Some(block.clone());
        }).or_insert_with(|| BlockStatus::with_block(block));
    }

    // Mark the block as validated; called when the BlockValid message is received by the engine
    pub fn block_valid(&mut self, block_id: &BlockId) {
        self.validator_backlog.entry(block_id.clone()).and_modify(|status| {
            status.block_valid = true;
        }).or_insert_with(BlockStatus::with_valid);
    }

    // Mark the block for commit; called when the engine wants to commit the block
    pub fn add_block_commit(&mut self, block_id: BlockId, entry: u64) {
        self.commit_queue.push_back((block_id, entry));
    }

    // Delete the current block (front of the queue) since it's been committed, and return its
    // corresponding entry
    pub fn block_committed(&mut self) -> u64 {
        let (block_id, entry) = self.commit_queue.pop_front().expect("No blocks in queue.");
        self.validator_backlog.remove(&block_id);
        entry
    }

    // Returns the BlockId of the next block that can be committed (if there is one)
    pub fn get_next_committable(&mut self, chain_head: &Block) -> Option<BlockId> {
        if let Some(&(ref block_id, _)) = self.commit_queue.front() {
            if let Some(status) = self.validator_backlog.get(&block_id) {
                if let Some(block) = status.block.clone() {
                    if status.block_valid && block.previous_id == chain_head.block_id {
                        // Block is ready
                        return Some(block_id.clone());
                    }
                }
            }
        }
        // No block or next block not ready
        None
    }
}
