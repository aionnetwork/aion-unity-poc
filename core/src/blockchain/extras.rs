/*******************************************************************************
 * Copyright (c) 2015-2018 Parity Technologies (UK) Ltd.
 * Copyright (c) 2018-2019 Aion foundation.
 *
 *     This file is part of the aion network project.
 *
 *     The aion network project is free software: you can redistribute it
 *     and/or modify it under the terms of the GNU General Public License
 *     as published by the Free Software Foundation, either version 3 of
 *     the License, or any later version.
 *
 *     The aion network project is distributed in the hope that it will
 *     be useful, but WITHOUT ANY WARRANTY; without even the implied
 *     warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.
 *     See the GNU General Public License for more details.
 *
 *     You should have received a copy of the GNU General Public License
 *     along with the aion network project source files.
 *     If not, see <https://www.gnu.org/licenses/>.
 *
 ******************************************************************************/

//! Blockchain DB extras.

use std::ops;
use std::io::Write;
use blooms::{GroupPosition, BloomGroup};
use db::Key;
use engines::epoch::{Transition as EpochTransition};
use header::BlockNumber;
use receipt::Receipt;

use heapsize::HeapSizeOf;
use aion_types::{H256, H264, U256};
use kvdb::PREFIX_LEN as DB_PREFIX_LEN;

/// Represents index of extra data in database
#[derive(Copy, Debug, Hash, Eq, PartialEq, Clone)]
pub enum ExtrasIndex {
    /// Block details index
    BlockDetails = 0,
    /// Block hash index
    BlockHash = 1,
    /// Transaction address index
    TransactionAddress = 2,
    /// Block blooms index
    BlocksBlooms = 3,
    /// Block receipts index
    BlockReceipts = 4,
    /// Epoch transition data index.
    EpochTransitions = 5,
    /// Pending epoch transition data index.
    PendingEpochTransition = 6,
}

fn with_index(hash: &H256, i: ExtrasIndex) -> H264 {
    let mut result = H264::default();
    result[0] = i as u8;
    (*result)[1..].clone_from_slice(hash);
    result
}

pub struct BlockNumberKey([u8; 5]);

impl ops::Deref for BlockNumberKey {
    type Target = [u8];

    fn deref(&self) -> &Self::Target { &self.0 }
}

impl Key<H256> for BlockNumber {
    type Target = BlockNumberKey;

    fn key(&self) -> Self::Target {
        let mut result = [0u8; 5];
        result[0] = ExtrasIndex::BlockHash as u8;
        result[1] = (self >> 24) as u8;
        result[2] = (self >> 16) as u8;
        result[3] = (self >> 8) as u8;
        result[4] = *self as u8;
        BlockNumberKey(result)
    }
}

impl Key<BlockDetails> for H256 {
    type Target = H264;

    fn key(&self) -> H264 { with_index(self, ExtrasIndex::BlockDetails) }
}

pub struct LogGroupKey([u8; 6]);

impl ops::Deref for LogGroupKey {
    type Target = [u8];

    fn deref(&self) -> &Self::Target { &self.0 }
}

impl Key<BloomGroup> for GroupPosition {
    type Target = LogGroupKey;

    fn key(&self) -> Self::Target {
        let mut result = [0u8; 6];
        result[0] = ExtrasIndex::BlocksBlooms as u8;
        result[1] = self.level;
        result[2] = (self.index >> 24) as u8;
        result[3] = (self.index >> 16) as u8;
        result[4] = (self.index >> 8) as u8;
        result[5] = self.index as u8;
        LogGroupKey(result)
    }
}

impl Key<TransactionAddress> for H256 {
    type Target = H264;

    fn key(&self) -> H264 { with_index(self, ExtrasIndex::TransactionAddress) }
}

impl Key<BlockReceipts> for H256 {
    type Target = H264;

    fn key(&self) -> H264 { with_index(self, ExtrasIndex::BlockReceipts) }
}

impl Key<::engines::epoch::PendingTransition> for H256 {
    type Target = H264;

    fn key(&self) -> H264 { with_index(self, ExtrasIndex::PendingEpochTransition) }
}

/// length of epoch keys.
pub const EPOCH_KEY_LEN: usize = DB_PREFIX_LEN + 16;

/// epoch key prefix.
/// used to iterate over all epoch transitions in order from genesis.
pub const EPOCH_KEY_PREFIX: &'static [u8; DB_PREFIX_LEN] = &[
    ExtrasIndex::EpochTransitions as u8,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
];

pub struct EpochTransitionsKey([u8; EPOCH_KEY_LEN]);

impl ops::Deref for EpochTransitionsKey {
    type Target = [u8];

    fn deref(&self) -> &[u8] { &self.0[..] }
}

impl Key<EpochTransitions> for u64 {
    type Target = EpochTransitionsKey;

    fn key(&self) -> Self::Target {
        let mut arr = [0u8; EPOCH_KEY_LEN];
        arr[..DB_PREFIX_LEN].copy_from_slice(&EPOCH_KEY_PREFIX[..]);

        write!(&mut arr[DB_PREFIX_LEN..], "{:016x}", self)
            .expect("format arg is valid; no more than 16 chars will be written; qed");

        EpochTransitionsKey(arr)
    }
}

/// Familial details concerning a block
#[derive(Debug, Clone, RlpEncodable, RlpDecodable)]
pub struct BlockDetails {
    /// Block number
    pub number: BlockNumber,
    /// Total mining difficulty of the block and all its parents
    pub total_pow_difficulty: U256,
    /// Total staking difficulty of the block and all its parents
    pub total_pos_difficulty: U256,
    /// Parent block hash
    pub parent: H256,
    /// List of children block hashes
    pub children: Vec<H256>,
    /// Block import time
    pub import_timestamp: u64,
}

impl HeapSizeOf for BlockDetails {
    fn heap_size_of_children(&self) -> usize { self.children.heap_size_of_children() }
}

/// Represents address of certain transaction within block
#[derive(Debug, PartialEq, Clone, RlpEncodable, RlpDecodable)]
pub struct TransactionAddress {
    /// Block hash
    pub block_hash: H256,
    /// Transaction index within the block
    pub index: usize,
}

impl HeapSizeOf for TransactionAddress {
    fn heap_size_of_children(&self) -> usize { 0 }
}

/// Contains all block receipts.
#[derive(Clone, RlpEncodableWrapper, RlpDecodableWrapper)]
pub struct BlockReceipts {
    pub receipts: Vec<Receipt>,
}

impl BlockReceipts {
    pub fn new(receipts: Vec<Receipt>) -> Self {
        BlockReceipts {
            receipts: receipts,
        }
    }
}

impl HeapSizeOf for BlockReceipts {
    fn heap_size_of_children(&self) -> usize { self.receipts.heap_size_of_children() }
}

/// Candidate transitions to an epoch with specific number.
#[derive(Clone, RlpEncodable, RlpDecodable)]
pub struct EpochTransitions {
    pub number: u64,
    pub candidates: Vec<EpochTransition>,
}

#[cfg(test)]
mod tests {
    use rlp::*;
    use super::BlockReceipts;

    #[test]
    fn encode_block_receipts() {
        let br = BlockReceipts::new(Vec::new());

        let mut s = RlpStream::new_list(2);
        s.append(&br);
        assert!(!s.is_finished(), "List shouldn't finished yet");
        s.append(&br);
        assert!(s.is_finished(), "List should be finished now");
        s.out();
    }
}
