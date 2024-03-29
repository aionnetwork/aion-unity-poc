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

//! A mutable state representation suitable to execute transactions.
//! Generic over a `Backend`. Deals with `Account`s.
//! Unconfirmed sub-states are managed with `checkpoint`s which may be canonicalized
//! or rolled back.

use blake2b::{BLAKE2B_EMPTY, BLAKE2B_NULL_RLP};
use std::cell::{RefCell, RefMut};
use std::collections::hash_map::Entry;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;
use std::sync::Arc;

use error::Error;
use executed::{Executed, ExecutionError};
use executive::Executive;
use factory::Factories;
use factory::VmFactory;
use machine::EthereumMachine as Machine;
use pod_account::*;
use pod_state::{self, PodState};
use receipt::Receipt;
use state_db::StateDB;
use transaction::SignedTransaction;
use types::basic_account::BasicAccount;
use types::state_diff::StateDiff;
use vms::EnvInfo;

use aion_types::{Address, H128, H256, U256};
use bytes::Bytes;
use kvdb::{KeyValueDB, AsHashStore, DBValue, HashStore, MemoryDBRepository};

use trie;
use trie::recorder::Recorder;
use trie::{Trie, TrieDB, TrieError};

mod account;
mod substate;

pub mod backend;

pub use self::account::Account;
pub use self::backend::Backend;
pub use self::substate::Substate;

/// Used to return information about an `State::apply` operation.
pub struct ApplyOutcome {
    /// The receipt for the applied transaction.
    pub receipt: Receipt,
}

/// Result type for the execution ("application") of a transaction.
pub type ApplyResult = Result<ApplyOutcome, Error>;

/// Return type of proof validity check.
#[derive(Debug, Clone)]
pub enum ProvedExecution {
    /// Proof wasn't enough to complete execution.
    BadProof,
    /// The transaction failed, but not due to a bad proof.
    Failed(ExecutionError),
    /// The transaction successfully completd with the given proof.
    Complete(Executed),
}

#[derive(Eq, PartialEq, Clone, Copy, Debug)]
/// Account modification state. Used to check if the account was
/// Modified in between commits and overall.
enum AccountState {
    /// Account was loaded from disk and never modified in this state object.
    CleanFresh,
    /// Account was loaded from the global cache and never modified.
    CleanCached,
    /// Account has been modified and is not committed to the trie yet.
    /// This is set if any of the account data is changed, including
    /// storage and code.
    Dirty,
    /// Account was modified and committed to the trie.
    Committed,
}

#[derive(Debug)]
/// In-memory copy of the account data. Holds the optional account
/// and the modification status.
/// Account entry can contain existing (`Some`) or non-existing
/// account (`None`)
struct AccountEntry {
    /// Account entry. `None` if account known to be non-existant.
    account: Option<Account>,
    /// Unmodified account balance.
    old_balance: Option<U256>,
    /// Entry state.
    state: AccountState,
}

// Account cache item. Contains account data and
// modification state
impl AccountEntry {
    fn is_dirty(&self) -> bool { self.state == AccountState::Dirty }

    /// Clone dirty data into new `AccountEntry`. This includes
    /// basic account data and modified storage keys.
    /// Returns None if clean.
    fn clone_if_dirty(&self) -> Option<AccountEntry> {
        match self.is_dirty() {
            true => Some(self.clone_dirty()),
            false => None,
        }
    }

    /// Clone dirty data into new `AccountEntry`. This includes
    /// basic account data and modified storage keys.
    fn clone_dirty(&self) -> AccountEntry {
        AccountEntry {
            old_balance: self.old_balance,
            account: self.account.as_ref().map(Account::clone_dirty),
            state: self.state,
        }
    }

    // Create a new account entry and mark it as dirty.
    fn new_dirty(account: Option<Account>) -> AccountEntry {
        AccountEntry {
            old_balance: account.as_ref().map(|a| a.balance().clone()),
            account: account,
            state: AccountState::Dirty,
        }
    }

    // Create a new account entry and mark it as clean.
    fn new_clean(account: Option<Account>) -> AccountEntry {
        AccountEntry {
            old_balance: account.as_ref().map(|a| a.balance().clone()),
            account: account,
            state: AccountState::CleanFresh,
        }
    }

    // Create a new account entry and mark it as clean and cached.
    fn new_clean_cached(account: Option<Account>) -> AccountEntry {
        AccountEntry {
            old_balance: account.as_ref().map(|a| a.balance().clone()),
            account: account,
            state: AccountState::CleanCached,
        }
    }

    // Replace data with another entry but preserve storage cache.
    fn overwrite_with(&mut self, other: AccountEntry) {
        self.state = other.state;
        match other.account {
            Some(acc) => {
                if let Some(ref mut ours) = self.account {
                    ours.overwrite_with(acc);
                }
            }
            None => self.account = None,
        }
    }
}

/// Check the given proof of execution.
/// `Err(ExecutionError::Internal)` indicates failure, everything else indicates
/// a successful proof (as the transaction itself may be poorly chosen).
pub fn check_proof(
    proof: &[DBValue],
    root: H256,
    transaction: &SignedTransaction,
    machine: &Machine,
    env_info: &EnvInfo,
) -> ProvedExecution
{
    let backend = self::backend::ProofCheck::new(proof);
    let mut factories = Factories::default();
    factories.accountdb = ::account_db::Factory::Plain;

    let res = State::from_existing(
        backend,
        root,
        machine.account_start_nonce(env_info.number),
        factories,
        Arc::new(MemoryDBRepository::new()),
    );

    let mut state = match res {
        Ok(state) => state,
        Err(_) => return ProvedExecution::BadProof,
    };

    match state.execute(env_info, machine, transaction, true, true) {
        Ok(executed) => ProvedExecution::Complete(executed),
        Err(ExecutionError::Internal(_)) => ProvedExecution::BadProof,
        Err(e) => ProvedExecution::Failed(e),
    }
}

/// Prove a transaction on the given state.
/// Returns `None` when the transacion could not be proved,
/// and a proof otherwise.
pub fn prove_transaction<H: AsHashStore + Send + Sync>(
    db: H,
    root: H256,
    transaction: &SignedTransaction,
    machine: &Machine,
    env_info: &EnvInfo,
    factories: Factories,
    virt: bool,
    kvdb: Arc<KeyValueDB>,
) -> Option<(Bytes, Vec<DBValue>)>
{
    use self::backend::Proving;

    let backend = Proving::new(db);
    let res = State::from_existing(
        backend,
        root,
        machine.account_start_nonce(env_info.number),
        factories,
        kvdb,
    );

    let mut state = match res {
        Ok(state) => state,
        Err(_) => return None,
    };

    match state.execute(env_info, machine, transaction, false, virt) {
        Err(ExecutionError::Internal(_)) => None,
        Err(e) => {
            trace!(target: "state", "Proved call failed: {}", e);
            Some((Vec::new(), state.drop().1.extract_proof()))
        }
        Ok(res) => Some((res.output, state.drop().1.extract_proof())),
    }
}

/// Representation of the entire state of all accounts in the system.
///
/// `State` can work together with `StateDB` to share account cache.
///
/// Local cache contains changes made locally and changes accumulated
/// locally from previous commits. Global cache reflects the database
/// state and never contains any changes.
///
/// Cache items contains account data, or the flag that account does not exist
/// and modification state (see `AccountState`)
///
/// Account data can be in the following cache states:
/// * In global but not local - something that was queried from the database,
/// but never modified
/// * In local but not global - something that was just added (e.g. new account)
/// * In both with the same value - something that was changed to a new value,
/// but changed back to a previous block in the same block (same State instance)
/// * In both with different values - something that was overwritten with a
/// new value.
///
/// All read-only state queries check local cache/modifications first,
/// then global state cache. If data is not found in any of the caches
/// it is loaded from the DB to the local cache.
///
/// **** IMPORTANT *************************************************************
/// All the modifications to the account data must set the `Dirty` state in the
/// `AccountEntry`. This is done in `require` and `require_or_from`. So just
/// use that.
/// ****************************************************************************
///
/// Upon destruction all the local cache data propagated into the global cache.
/// Propagated items might be rejected if current state is non-canonical.
///
/// State checkpointing.
///
/// A new checkpoint can be created with `checkpoint()`. checkpoints can be
/// created in a hierarchy.
/// When a checkpoint is active all changes are applied directly into
/// `cache` and the original value is copied into an active checkpoint.
/// Reverting a checkpoint with `revert_to_checkpoint` involves copying
/// original values from the latest checkpoint back into `cache`. The code
/// takes care not to overwrite cached storage while doing that.
/// checkpoint can be discarded with `discard_checkpoint`. All of the orignal
/// backed-up values are moved into a parent checkpoint (if any).
///
pub struct State<B: Backend> {
    db: B,
    root: H256,
    cache: RefCell<HashMap<Address, AccountEntry>>,
    // The original account is preserved in
    checkpoints: RefCell<Vec<HashMap<Address, Option<AccountEntry>>>>,
    account_start_nonce: U256,
    factories: Factories,
    kvdb: Arc<KeyValueDB>,
}

#[derive(Copy, Clone)]
enum RequireCache {
    None,
    CodeSize,
    Code,
}

/// Mode of dealing with null accounts.
#[derive(PartialEq)]
pub enum CleanupMode<'a> {
    /// Create accounts which would be null.
    ForceCreate,
    /// Don't delete null accounts upon touching, but also don't create them.
    NoEmpty,
    /// Mark all touched accounts.
    TrackTouched(&'a mut HashSet<Address>),
}

const SEC_TRIE_DB_UNWRAP_STR: &'static str =
    "A state can only be created with valid root. Creating a SecTrieDB with a valid root will not \
     fail. Therefore creating a SecTrieDB with this state's root will not fail.";

impl<B: Backend> State<B> {
    /// Creates new state with empty state root
    /// Used for tests.
    pub fn new(
        mut db: B,
        account_start_nonce: U256,
        factories: Factories,
        kvdb: Arc<KeyValueDB>,
    ) -> State<B>
    {
        let mut root = H256::new();
        {
            // init trie and reset root too null
            let _ = factories.trie.create(db.as_hashstore_mut(), &mut root);
        }

        State {
            db: db,
            root: root,
            cache: RefCell::new(HashMap::new()),
            checkpoints: RefCell::new(Vec::new()),
            account_start_nonce: account_start_nonce,
            factories: factories,
            kvdb: kvdb,
        }
    }

    /// Creates new state with existing state root
    pub fn from_existing(
        db: B,
        root: H256,
        account_start_nonce: U256,
        factories: Factories,
        kvdb: Arc<KeyValueDB>,
    ) -> Result<State<B>, TrieError>
    {
        if !db.as_hashstore().contains(&root) {
            return Err(TrieError::InvalidStateRoot(root));
        }

        let state = State {
            db: db,
            root: root,
            cache: RefCell::new(HashMap::new()),
            checkpoints: RefCell::new(Vec::new()),
            account_start_nonce: account_start_nonce,
            factories: factories,
            kvdb: kvdb,
        };

        Ok(state)
    }

    pub fn export_kvdb(&self) -> Arc<KeyValueDB> { self.kvdb.clone() }

    /// Get a VM factory that can execute on this state.
    pub fn vm_factory(&self) -> VmFactory { self.factories.vm.clone() }

    /// Swap the current backend for another.
    // TODO: [rob] find a less hacky way to avoid duplication of `Client::state_at`.
    pub fn replace_backend<T: Backend>(self, backend: T) -> State<T> {
        State {
            db: backend,
            root: self.root,
            cache: self.cache,
            checkpoints: self.checkpoints,
            account_start_nonce: self.account_start_nonce,
            factories: self.factories,
            kvdb: self.kvdb,
        }
    }

    /// Create a recoverable checkpoint of this state.
    pub fn checkpoint(&mut self) { self.checkpoints.get_mut().push(HashMap::new()); }

    /// Merge last checkpoint with previous.
    pub fn discard_checkpoint(&mut self) {
        // merge with previous checkpoint
        let last = self.checkpoints.get_mut().pop();
        if let Some(mut checkpoint) = last {
            if let Some(ref mut prev) = self.checkpoints.get_mut().last_mut() {
                if prev.is_empty() {
                    **prev = checkpoint;
                } else {
                    for (k, v) in checkpoint.drain() {
                        prev.entry(k).or_insert(v);
                    }
                }
            }
        }
    }

    /// Revert to the last checkpoint and discard it.
    pub fn revert_to_checkpoint(&mut self) {
        if let Some(mut checkpoint) = self.checkpoints.get_mut().pop() {
            for (k, v) in checkpoint.drain() {
                match v {
                    Some(v) => {
                        match self.cache.get_mut().entry(k) {
                            Entry::Occupied(mut e) => {
                                // Merge checkpointed changes back into the main account
                                // storage preserving the cache.
                                e.get_mut().overwrite_with(v);
                            }
                            Entry::Vacant(e) => {
                                e.insert(v);
                            }
                        }
                    }
                    None => {
                        if let Entry::Occupied(e) = self.cache.get_mut().entry(k) {
                            if e.get().is_dirty() {
                                e.remove();
                            }
                        }
                    }
                }
            }
        }
    }

    fn insert_cache(&self, address: &Address, account: AccountEntry) {
        // Dirty account which is not in the cache means this is a new account.
        // It goes directly into the checkpoint as there's nothing to rever to.
        //
        // In all other cases account is read as clean first, and after that made
        // dirty in and added to the checkpoint with `note_cache`.
        let is_dirty = account.is_dirty();
        let old_value = self.cache.borrow_mut().insert(*address, account);
        if is_dirty {
            if let Some(ref mut checkpoint) = self.checkpoints.borrow_mut().last_mut() {
                checkpoint.entry(*address).or_insert(old_value);
            }
        }
    }

    fn note_cache(&self, address: &Address) {
        if let Some(ref mut checkpoint) = self.checkpoints.borrow_mut().last_mut() {
            checkpoint.entry(*address).or_insert_with(|| {
                self.cache
                    .borrow()
                    .get(address)
                    .map(AccountEntry::clone_dirty)
            });
        }
    }

    /// Destroy the current object and return root and database.
    pub fn drop(mut self) -> (H256, B) {
        self.propagate_to_global_cache();
        (self.root, self.db)
    }

    /// Return reference to root
    pub fn root(&self) -> &H256 { &self.root }

    /// Create a new contract at address `contract`. If there is already an account at the address
    /// it will have its code reset, ready for `init_code()`.
    pub fn new_contract(&mut self, contract: &Address, balance: U256, nonce_offset: U256) {
        self.insert_cache(
            contract,
            AccountEntry::new_dirty(Some(Account::new_contract(
                balance,
                self.account_start_nonce + nonce_offset,
            ))),
        );
    }

    /// Remove an existing account.
    pub fn kill_account(&mut self, account: &Address) {
        self.insert_cache(account, AccountEntry::new_dirty(None));
    }

    /// Determine whether an account exists.
    pub fn exists(&self, a: &Address) -> trie::Result<bool> {
        // Bloom filter does not contain empty accounts, so it is important here to
        // check if account exists in the database directly before EIP-161 is in effect.
        self.ensure_cached(a, RequireCache::None, false, |a| a.is_some())
    }

    /// Determine whether an account exists and if not empty.
    pub fn exists_and_not_null(&self, a: &Address) -> trie::Result<bool> {
        self.ensure_cached(a, RequireCache::None, false, |a| {
            a.map_or(false, |a| !a.is_null())
        })
    }

    /// Determine whether an account exists and has code or non-zero nonce.
    pub fn exists_and_has_code_or_nonce(&self, a: &Address) -> trie::Result<bool> {
        self.ensure_cached(a, RequireCache::CodeSize, false, |a| {
            a.map_or(false, |a| {
                a.code_hash() != BLAKE2B_EMPTY || *a.nonce() != self.account_start_nonce
            })
        })
    }

    /// Get the balance of account `a`.
    pub fn balance(&self, a: &Address) -> trie::Result<U256> {
        self.ensure_cached(a, RequireCache::None, true, |a| {
            a.as_ref()
                .map_or(U256::zero(), |account| *account.balance())
        })
    }

    /// Get the nonce of account `a`.
    pub fn nonce(&self, a: &Address) -> trie::Result<U256> {
        self.ensure_cached(a, RequireCache::None, true, |a| {
            a.as_ref()
                .map_or(self.account_start_nonce, |account| *account.nonce())
        })
    }

    /// Get the storage root of account `a`.
    pub fn storage_root(&self, a: &Address) -> trie::Result<Option<H256>> {
        self.ensure_cached(a, RequireCache::None, true, |a| {
            a.as_ref()
                .and_then(|account| account.storage_root().cloned())
        })
    }

    /// Mutate storage of account `address` so that it is `value` for `key`.
    pub fn storage_at(&self, address: &Address, key: &H128) -> trie::Result<H128> {
        // Storage key search and update works like this:
        // 1. If there's an entry for the account in the local cache check for the key and return it if found.
        // 2. If there's an entry for the account in the global cache check for the key or load it into that account.
        // 3. If account is missing in the global cache load it into the local cache and cache the key there.

        trace!("address = {}, key = {:x}", address, key);

        // check local cache first without updating
        {
            let local_cache = self.cache.borrow_mut();
            let mut local_account = None;
            if let Some(maybe_acc) = local_cache.get(address) {
                match maybe_acc.account {
                    Some(ref account) => {
                        if let Some(value) = account.cached_storage_at(key) {
                            return Ok(value);
                        } else {
                            local_account = Some(maybe_acc);
                        }
                    }
                    _ => return Ok(H128::new()),
                }
            }
            // check the global cache and and cache storage key there if found,
            let trie_res = self.db.get_cached(address, |acc| {
                match acc {
                    None => Ok(H128::new()),
                    Some(a) => {
                        let account_db = self
                            .factories
                            .accountdb
                            .readonly(self.db.as_hashstore(), a.address_hash(address));
                        a.storage_at(account_db.as_hashstore(), key)
                    }
                }
            });

            if let Some(res) = trie_res {
                return res;
            }

            // otherwise cache the account localy and cache storage key there.
            if let Some(ref mut acc) = local_account {
                if let Some(ref account) = acc.account {
                    let account_db = self
                        .factories
                        .accountdb
                        .readonly(self.db.as_hashstore(), account.address_hash(address));
                    return account.storage_at(account_db.as_hashstore(), key);
                } else {
                    return Ok(H128::new());
                }
            }
        }

        // check if the account could exist before any requests to trie
        if self.db.is_known_null(address) {
            return Ok(H128::zero());
        }

        // account is not found in the global cache, get from the DB and insert into local
        let db = self
            .factories
            .trie
            .readonly(self.db.as_hashstore(), &self.root)
            .expect(SEC_TRIE_DB_UNWRAP_STR);
        let maybe_acc = db.get_with(address, Account::from_rlp)?;
        let r = maybe_acc.as_ref().map_or(Ok(H128::new()), |a| {
            let account_db = self
                .factories
                .accountdb
                .readonly(self.db.as_hashstore(), a.address_hash(address));
            a.storage_at(account_db.as_hashstore(), key)
        });
        self.insert_cache(address, AccountEntry::new_clean(maybe_acc));
        r
    }

    pub fn storage_at_dword(&self, address: &Address, key: &H128) -> trie::Result<H256> {
        {
            let local_cache = self.cache.borrow_mut();
            let mut local_account = None;
            if let Some(maybe_acc) = local_cache.get(address) {
                match maybe_acc.account {
                    Some(ref account) => {
                        if let Some(value) = account.cached_storage_at_dword(key) {
                            return Ok(value);
                        } else {
                            local_account = Some(maybe_acc);
                        }
                    }
                    _ => return Ok(H256::new()),
                }
            }
            // check the global cache and and cache storage key there if found,
            let trie_res = self.db.get_cached(address, |acc| {
                match acc {
                    None => Ok(H256::new()),
                    Some(a) => {
                        let account_db = self
                            .factories
                            .accountdb
                            .readonly(self.db.as_hashstore(), a.address_hash(address));
                        a.storage_at_dword(account_db.as_hashstore(), key)
                    }
                }
            });

            if let Some(res) = trie_res {
                return res;
            }

            // otherwise cache the account localy and cache storage key there.
            if let Some(ref mut acc) = local_account {
                if let Some(ref account) = acc.account {
                    let account_db = self
                        .factories
                        .accountdb
                        .readonly(self.db.as_hashstore(), account.address_hash(address));
                    return account.storage_at_dword(account_db.as_hashstore(), key);
                } else {
                    return Ok(H256::new());
                }
            }
        }

        // check if the account could exist before any requests to trie
        if self.db.is_known_null(address) {
            return Ok(H256::zero());
        }

        // account is not found in the global cache, get from the DB and insert into local
        let db = self
            .factories
            .trie
            .readonly(self.db.as_hashstore(), &self.root)
            .expect(SEC_TRIE_DB_UNWRAP_STR);
        let maybe_acc = db.get_with(address, Account::from_rlp)?;
        let r = maybe_acc.as_ref().map_or(Ok(H256::new()), |a| {
            let account_db = self
                .factories
                .accountdb
                .readonly(self.db.as_hashstore(), a.address_hash(address));
            a.storage_at_dword(account_db.as_hashstore(), key)
        });
        self.insert_cache(address, AccountEntry::new_clean(maybe_acc));
        r
    }

    /// Get accounts' code.
    pub fn code(&self, a: &Address) -> trie::Result<Option<Arc<Bytes>>> {
        self.ensure_cached(a, RequireCache::Code, true, |a| {
            a.as_ref().map_or(None, |a| a.code().clone())
        })
    }

    /// Get an account's code hash.
    pub fn code_hash(&self, a: &Address) -> trie::Result<H256> {
        self.ensure_cached(a, RequireCache::None, true, |a| {
            a.as_ref().map_or(BLAKE2B_EMPTY, |a| a.code_hash())
        })
    }

    /// Get accounts' code size.
    pub fn code_size(&self, a: &Address) -> trie::Result<Option<usize>> {
        self.ensure_cached(a, RequireCache::CodeSize, true, |a| {
            a.as_ref().and_then(|a| a.code_size())
        })
    }

    /// Add `incr` to the balance of account `a`.
    pub fn add_balance(
        &mut self,
        a: &Address,
        incr: &U256,
        cleanup_mode: CleanupMode,
    ) -> trie::Result<()>
    {
        debug!(target: "state", "add_balance({}, {}): {}", a, incr, self.balance(a)?);
        let is_value_transfer = !incr.is_zero();
        if is_value_transfer || (cleanup_mode == CleanupMode::ForceCreate && !self.exists(a)?) {
            self.require(a, false)?.add_balance(incr);
        } else if let CleanupMode::TrackTouched(set) = cleanup_mode {
            if self.exists(a)? {
                set.insert(*a);
                self.touch(a)?;
            }
        }
        Ok(())
    }

    /// Subtract `decr` from the balance of account `a`.
    pub fn sub_balance(
        &mut self,
        a: &Address,
        decr: &U256,
        cleanup_mode: &mut CleanupMode,
    ) -> trie::Result<()>
    {
        debug!(target: "state", "sub_balance({}, {}): {}", a, decr, self.balance(a)?);
        if !decr.is_zero() || !self.exists(a)? {
            self.require(a, false)?.sub_balance(decr);
        }
        if let CleanupMode::TrackTouched(ref mut set) = *cleanup_mode {
            set.insert(*a);
        }
        Ok(())
    }

    /// Subtracts `by` from the balance of `from` and adds it to that of `to`.
    pub fn transfer_balance(
        &mut self,
        from: &Address,
        to: &Address,
        by: &U256,
        mut cleanup_mode: CleanupMode,
    ) -> trie::Result<()>
    {
        self.sub_balance(from, by, &mut cleanup_mode)?;
        self.add_balance(to, by, cleanup_mode)?;
        Ok(())
    }

    /// Increment the nonce of account `a` by 1.
    pub fn inc_nonce(&mut self, a: &Address) -> trie::Result<()> {
        self.require(a, false).map(|mut x| x.inc_nonce())
    }

    /// Mutate storage of account `a` so that it is `value` for `key`.
    pub fn set_storage(&mut self, a: &Address, key: H128, value: H128) -> trie::Result<()> {
        trace!(target: "state", "set_storage({}:{:x} to {:x})", a, key, value);
        self.require(a, false)?.set_storage(key, value);
        Ok(())
    }

    /// Mutate storage of account `a` so that it is `value` for `key`.
    pub fn set_storage_dword(&mut self, a: &Address, key: H128, value: H256) -> trie::Result<()> {
        trace!(target: "state", "set_storage({}:{:x} to {:x})", a, key, value);
        self.require(a, false)?.set_storage_dword(key, value);
        Ok(())
    }

    /// Initialise the code of account `a` so that it is `code`.
    /// NOTE: Account should have been created with `new_contract`.
    pub fn init_code(&mut self, a: &Address, code: Bytes) -> trie::Result<()> {
        self.require_or_from(
            a,
            true,
            || Account::new_contract(0.into(), self.account_start_nonce),
            |_| {},
        )?
        .init_code(code);
        Ok(())
    }

    pub fn set_empty_but_commit(&mut self, a: &Address) -> trie::Result<()> {
        self.require_or_from(
            a,
            true,
            || Account::new_contract(0.into(), self.account_start_nonce),
            |_| {},
        )?
        .set_empty_but_commit();
        Ok(())
    }

    /// Reset the code of account `a` so that it is `code`.
    pub fn reset_code(&mut self, a: &Address, code: Bytes) -> trie::Result<()> {
        self.require_or_from(
            a,
            true,
            || Account::new_contract(0.into(), self.account_start_nonce),
            |_| {},
        )?
        .reset_code(code);
        Ok(())
    }

    /// Execute a given transaction, producing a receipt.
    /// This will change the state accordingly.
    pub fn apply(
        &mut self,
        env_info: &EnvInfo,
        machine: &Machine,
        t: &SignedTransaction,
    ) -> ApplyResult
    {
        let e = self.execute(env_info, machine, t, true, false)?;

        self.commit()?;
        let state_root = self.root().clone();

        let receipt = Receipt::new(
            state_root,
            e.gas_used,
            e.transaction_fee,
            e.logs,
            e.output,
            e.exception,
        );
        trace!(target: "state", "Transaction receipt: {:?}", receipt);

        Ok(ApplyOutcome {
            receipt,
        })
    }

    // Execute a given transaction without committing changes.
    //
    // `virt` signals that we are executing outside of a block set and restrictions like
    // gas limits and gas costs should be lifted.
    fn execute(
        &mut self,
        env_info: &EnvInfo,
        machine: &Machine,
        t: &SignedTransaction,
        check_nonce: bool,
        virt: bool,
    ) -> Result<Executed, ExecutionError>
    {
        let mut e = Executive::new(self, env_info, machine);

        match virt {
            true => e.transact_virtual(t, check_nonce),
            false => e.transact(t, check_nonce, false),
        }
    }

    fn touch(&mut self, a: &Address) -> trie::Result<()> {
        self.require(a, false)?;
        Ok(())
    }

    /// Commits our cached account changes into the trie.
    pub fn commit(&mut self) -> Result<(), Error> {
        // first, commit the sub trees.
        let mut accounts = self.cache.borrow_mut();
        debug!(target: "cons", "commit accounts = {:?}", accounts);
        for (address, ref mut a) in accounts.iter_mut().filter(|&(_, ref a)| a.is_dirty()) {
            if let Some(ref mut account) = a.account {
                let addr_hash = account.address_hash(address);
                {
                    let mut account_db = self
                        .factories
                        .accountdb
                        .create(self.db.as_hashstore_mut(), addr_hash);
                    account.commit_code(account_db.as_hashstore_mut());
                    // Tmp workaround to ignore storage changes on null accounts
                    // until java kernel fixed the problem
                    if !account.is_null()
                        || address == &H256::from(
                            "0000000000000000000000000000000000000000000000000000000000000100",
                        )
                        || address == &H256::from(
                            "0000000000000000000000000000000000000000000000000000000000000200",
                        ) {
                        account
                            .commit_storage(&self.factories.trie, account_db.as_hashstore_mut())?;
                        account.commit_storage_dword(
                            &self.factories.trie,
                            account_db.as_hashstore_mut(),
                        )?;
                    } else if !account.storage_changes().is_empty()
                        || !account.storage_changes_dword().is_empty()
                    {
                        account.discard_storage_changes();
                        a.state = AccountState::CleanFresh;
                    } else {
                        if a.state == AccountState::Dirty
                            && account.code_hash() == BLAKE2B_EMPTY
                            && !account.get_empty_but_commit()
                        {
                            // Aion Java Kernel specific:
                            // 1. for code != NULL && return code == NULL && no storage chanage
                            // eg: [0x00, 0x60, 0x00]
                            // 2. code is NULL, this account should be commited
                            a.state = AccountState::CleanFresh;
                        }
                    }
                }
                if !account.is_empty() {
                    self.db.note_non_null_account(address);
                }
            }
        }

        {
            let mut trie = self
                .factories
                .trie
                .from_existing(self.db.as_hashstore_mut(), &mut self.root)?;
            for (address, ref mut a) in accounts.iter_mut().filter(|&(_, ref a)| a.is_dirty()) {
                a.state = AccountState::Committed;
                match a.account {
                    Some(ref mut account) => {
                        trie.insert(address, &account.rlp())?;
                    }
                    None => {
                        trie.remove(address)?;
                    }
                };
            }
        }
        debug!(target: "cons", "after commit accounts = {:?}", accounts);

        Ok(())
    }

    /// Propagate local cache into shared canonical state cache.
    fn propagate_to_global_cache(&mut self) {
        let mut addresses = self.cache.borrow_mut();
        trace!(target:"state","Committing cache {:?} entries", addresses.len());
        for (address, a) in addresses.drain().filter(|&(_, ref a)| {
            a.state == AccountState::Committed || a.state == AccountState::CleanFresh
        }) {
            self.db
                .add_to_account_cache(address, a.account, a.state == AccountState::Committed);
        }
    }

    /// Clear state cache
    pub fn clear(&mut self) { self.cache.borrow_mut().clear(); }

    /// Populate the state from `accounts`.
    /// Used for tests.
    pub fn populate_from(&mut self, accounts: PodState) {
        assert!(self.checkpoints.borrow().is_empty());
        for (add, acc) in accounts.drain().into_iter() {
            self.cache
                .borrow_mut()
                .insert(add, AccountEntry::new_dirty(Some(Account::from_pod(acc))));
        }
    }

    /// Populate a PodAccount map from this state.
    pub fn to_pod(&self) -> PodState {
        assert!(self.checkpoints.borrow().is_empty());
        // TODO: handle database rather than just the cache.
        // will need fat db.
        PodState::from(
            self.cache
                .borrow()
                .iter()
                .fold(BTreeMap::new(), |mut m, (add, opt)| {
                    if let Some(ref acc) = opt.account {
                        m.insert(add.clone(), PodAccount::from_account(acc));
                    }
                    m
                }),
        )
    }

    // Return a list of all touched addresses in cache.
    fn touched_addresses(&self) -> Vec<Address> {
        assert!(self.checkpoints.borrow().is_empty());
        self.cache.borrow().iter().map(|(add, _)| *add).collect()
    }

    fn query_pod(&mut self, query: &PodState, touched_addresses: &[Address]) -> trie::Result<()> {
        let pod = query.get();

        for address in touched_addresses {
            if !self.ensure_cached(address, RequireCache::Code, true, |a| a.is_some())? {
                continue;
            }

            if let Some(pod_account) = pod.get(address) {
                // needs to be split into two parts for the refcell code here
                // to work.
                for key in pod_account.storage.keys() {
                    self.storage_at(address, key)?;
                }

                for key in pod_account.storage_dword.keys() {
                    self.storage_at_dword(address, key)?;
                }
            }
        }

        Ok(())
    }

    /// Returns a `StateDiff` describing the difference from `orig` to `self`.
    /// Consumes self.
    pub fn diff_from<X: Backend>(&self, orig: State<X>) -> trie::Result<StateDiff> {
        let addresses_post = self.touched_addresses();
        let pod_state_post = self.to_pod();
        let mut state_pre = orig;
        state_pre.query_pod(&pod_state_post, &addresses_post)?;
        Ok(pod_state::diff_pod(&state_pre.to_pod(), &pod_state_post))
    }

    // load required account data from the databases.
    fn update_account_cache(
        require: RequireCache,
        account: &mut Account,
        state_db: &B,
        db: &HashStore,
    )
    {
        if let RequireCache::None = require {
            return;
        }

        if account.is_cached() {
            return;
        }

        // if there's already code in the global cache, always cache it localy
        let hash = account.code_hash();
        match state_db.get_cached_code(&hash) {
            Some(code) => account.cache_given_code(code),
            None => {
                match require {
                    RequireCache::None => {}
                    RequireCache::Code => {
                        if let Some(code) = account.cache_code(db) {
                            // propagate code loaded from the database to
                            // the global code cache.
                            state_db.cache_code(hash, code)
                        }
                    }
                    RequireCache::CodeSize => {
                        account.cache_code_size(db);
                    }
                }
            }
        }
    }

    /// Check caches for required data
    /// First searches for account in the local, then the shared cache.
    /// Populates local cache if nothing found.
    fn ensure_cached<F, U>(
        &self,
        a: &Address,
        require: RequireCache,
        check_null: bool,
        f: F,
    ) -> trie::Result<U>
    where
        F: Fn(Option<&Account>) -> U,
    {
        // check local cache first
        if let Some(ref mut maybe_acc) = self.cache.borrow_mut().get_mut(a) {
            if let Some(ref mut account) = maybe_acc.account {
                let accountdb = self
                    .factories
                    .accountdb
                    .readonly(self.db.as_hashstore(), account.address_hash(a));
                Self::update_account_cache(require, account, &self.db, accountdb.as_hashstore());
                return Ok(f(Some(account)));
            }
            return Ok(f(None));
        }
        // check global cache
        let result = self.db.get_cached(a, |mut acc| {
            if let Some(ref mut account) = acc {
                let accountdb = self
                    .factories
                    .accountdb
                    .readonly(self.db.as_hashstore(), account.address_hash(a));
                Self::update_account_cache(require, account, &self.db, accountdb.as_hashstore());
            }
            f(acc.map(|a| &*a))
        });
        match result {
            Some(r) => Ok(r),
            None => {
                // first check if it is not in database for sure
                if check_null && self.db.is_known_null(a) {
                    return Ok(f(None));
                }

                // not found in the global cache, get from the DB and insert into local
                let db = self
                    .factories
                    .trie
                    .readonly(self.db.as_hashstore(), &self.root)?;
                let mut maybe_acc = db.get_with(a, Account::from_rlp)?;
                if let Some(ref mut account) = maybe_acc.as_mut() {
                    let accountdb = self
                        .factories
                        .accountdb
                        .readonly(self.db.as_hashstore(), account.address_hash(a));
                    Self::update_account_cache(
                        require,
                        account,
                        &self.db,
                        accountdb.as_hashstore(),
                    );
                }
                let r = f(maybe_acc.as_ref());
                self.insert_cache(a, AccountEntry::new_clean(maybe_acc));
                Ok(r)
            }
        }
    }

    /// Pull account `a` in our cache from the trie DB. `require_code` requires that the code be cached, too.
    fn require<'a>(&'a self, a: &Address, require_code: bool) -> trie::Result<RefMut<'a, Account>> {
        self.require_or_from(
            a,
            require_code,
            || Account::new_basic(0u8.into(), self.account_start_nonce),
            |_| {},
        )
    }

    /// Pull account `a` in our cache from the trie DB. `require_code` requires that the code be cached, too.
    /// If it doesn't exist, make account equal the evaluation of `default`.
    fn require_or_from<'a, F, G>(
        &'a self,
        a: &Address,
        require_code: bool,
        default: F,
        not_default: G,
    ) -> trie::Result<RefMut<'a, Account>>
    where
        F: FnOnce() -> Account,
        G: FnOnce(&mut Account),
    {
        let contains_key = self.cache.borrow().contains_key(a);
        if !contains_key {
            match self.db.get_cached_account(a) {
                Some(acc) => self.insert_cache(a, AccountEntry::new_clean_cached(acc)),
                None => {
                    let maybe_acc = if !self.db.is_known_null(a) {
                        let db = self
                            .factories
                            .trie
                            .readonly(self.db.as_hashstore(), &self.root)?;
                        AccountEntry::new_clean(db.get_with(a, Account::from_rlp)?)
                    } else {
                        AccountEntry::new_clean(None)
                    };
                    self.insert_cache(a, maybe_acc);
                }
            }
        }
        self.note_cache(a);

        // at this point the entry is guaranteed to be in the cache.
        Ok(RefMut::map(self.cache.borrow_mut(), |c| {
            let entry = c
                .get_mut(a)
                .expect("entry known to exist in the cache; qed");

            match &mut entry.account {
                &mut Some(ref mut acc) => not_default(acc),
                slot => *slot = Some(default()),
            }

            // set the dirty flag after changing account data.
            entry.state = AccountState::Dirty;
            match entry.account {
                Some(ref mut account) => {
                    if require_code {
                        let addr_hash = account.address_hash(a);
                        let accountdb = self
                            .factories
                            .accountdb
                            .readonly(self.db.as_hashstore(), addr_hash);
                        Self::update_account_cache(
                            RequireCache::Code,
                            account,
                            &self.db,
                            accountdb.as_hashstore(),
                        );
                    }
                    account
                }
                _ => panic!("Required account must always exist; qed"),
            }
        }))
    }
}

// State proof implementations; useful for light client protocols.
impl<B: Backend> State<B> {
    /// Prove an account's existence or nonexistence in the state trie.
    /// Returns a merkle proof of the account's trie node omitted or an encountered trie error.
    /// If the account doesn't exist in the trie, prove that and return defaults.
    /// Requires a secure trie to be used for accurate results.
    /// `account_key` == blake2b(address)
    pub fn prove_account(&self, account_key: H256) -> trie::Result<(Vec<Bytes>, BasicAccount)> {
        let mut recorder = Recorder::new();
        let trie = TrieDB::new(self.db.as_hashstore(), &self.root)?;
        let maybe_account: Option<BasicAccount> = {
            let query = (&mut recorder, ::rlp::decode);
            trie.get_with(&account_key, query)?
        };
        let account = maybe_account.unwrap_or_else(|| {
            BasicAccount {
                balance: 0.into(),
                nonce: self.account_start_nonce,
                code_hash: BLAKE2B_EMPTY,
                storage_root: BLAKE2B_NULL_RLP,
            }
        });

        Ok((
            recorder.drain().into_iter().map(|r| r.data).collect(),
            account,
        ))
    }

    /// Prove an account's storage key's existence or nonexistence in the state.
    /// Returns a merkle proof of the account's storage trie.
    /// Requires a secure trie to be used for correctness.
    /// `account_key` == blake2b(address)
    /// `storage_key` == blake2b(key)
    pub fn prove_storage(
        &self,
        account_key: H256,
        storage_key: H256,
    ) -> trie::Result<(Vec<Bytes>, H256)>
    {
        // TODO: probably could look into cache somehow but it's keyed by
        // address, not blake2b(address).
        let trie = TrieDB::new(self.db.as_hashstore(), &self.root)?;
        let acc = match trie.get_with(&account_key, Account::from_rlp)? {
            Some(acc) => acc,
            None => return Ok((Vec::new(), H256::new())),
        };

        let account_db = self
            .factories
            .accountdb
            .readonly(self.db.as_hashstore(), account_key);
        acc.prove_storage(account_db.as_hashstore(), storage_key)
    }
}

impl<B: Backend> fmt::Debug for State<B> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{:?}", self.cache.borrow()) }
}

// TODO: cloning for `State` shouldn't be possible in general; Remove this and use
// checkpoints where possible.
impl Clone for State<StateDB> {
    fn clone(&self) -> State<StateDB> {
        let cache = {
            let mut cache: HashMap<Address, AccountEntry> = HashMap::new();
            for (key, val) in self.cache.borrow().iter() {
                if let Some(entry) = val.clone_if_dirty() {
                    cache.insert(key.clone(), entry);
                }
            }
            cache
        };

        State {
            db: self.db.boxed_clone(),
            root: self.root.clone(),
            cache: RefCell::new(cache),
            checkpoints: RefCell::new(Vec::new()),
            account_start_nonce: self.account_start_nonce.clone(),
            factories: self.factories.clone(),
            kvdb: self.kvdb.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aion_types::{Address, H256, U128, U256};
    use key::Ed25519Secret;
    use logger::init_log;
    use receipt::SimpleReceipt;
    use rustc_hex::FromHex;
    use std::str::FromStr;
    use tests::helpers::*;
    use transaction::*;

    fn secret() -> Ed25519Secret {
        Ed25519Secret::from_str("7ea8af7d0982509cd815096d35bc3a295f57b2a078e4e25731e3ea977b9544626702b86f33072a55f46003b1e3e242eb18556be54c5ab12044c3c20829e0abb5").unwrap()
    }

    fn make_frontier_machine() -> Machine {
        let machine = ::ethereum::new_frontier_test_machine();
        machine
    }

    #[test]
    fn should_apply_create_transaction() {
        init_log();

        let mut state = get_temp_state();
        let mut info = EnvInfo::default();
        info.gas_limit = 1_000_000.into();
        let machine = make_frontier_machine();

        let t = Transaction {
            nonce: 0.into(),
            nonce_bytes: Vec::new(),
            gas_price: 0.into(),
            gas_price_bytes: Vec::new(),
            gas: 500_000.into(),
            gas_bytes: Vec::new(),
            action: Action::Create,
            value: 100.into(),
            value_bytes: Vec::new(),
            transaction_type: 1,
            data: FromHex::from_hex("601080600c6000396000f3006000355415600957005b60203560003555")
                .unwrap(),
        }
        .sign(&secret(), None);

        state
            .add_balance(&t.sender(), &(100.into()), CleanupMode::NoEmpty)
            .unwrap();
        let result = state.apply(&info, &machine, &t).unwrap();

        let expected_receipt = Receipt {
            simple_receipt: SimpleReceipt{log_bloom: "0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000".into(),
            logs: vec![], state_root: H256::from(
                    "0xadfb0633de8b1effff5c6b4f347b435f99e48339164160ee04bac13115c90dc9"
                ), },
            output: vec![96, 0, 53, 84, 21, 96, 9, 87, 0, 91, 96, 32, 53, 96, 0, 53],
            gas_used: U256::from(222506),
            error_message:  String::new(),
            transaction_fee: U256::from(0),
        };

        assert_eq!(result.receipt, expected_receipt);
    }

    #[test]
    fn should_work_when_cloned() {
        init_log();

        let a = Address::zero();

        let mut state = {
            let mut state = get_temp_state();
            assert_eq!(state.exists(&a).unwrap(), false);
            state.inc_nonce(&a).unwrap();
            state.commit().unwrap();
            state.clone()
        };

        state.inc_nonce(&a).unwrap();
        state.commit().unwrap();
    }

    #[test]
    fn code_from_database() {
        let a = Address::zero();
        let (root, db) = {
            let mut state = get_temp_state();
            state
                .require_or_from(
                    &a,
                    false,
                    || Account::new_contract(42.into(), 0.into()),
                    |_| {},
                )
                .unwrap();
            state.init_code(&a, vec![1, 2, 3]).unwrap();
            assert_eq!(state.code(&a).unwrap(), Some(Arc::new(vec![1u8, 2, 3])));
            state.commit().unwrap();
            assert_eq!(state.code(&a).unwrap(), Some(Arc::new(vec![1u8, 2, 3])));
            state.drop()
        };

        let state = State::from_existing(
            db,
            root,
            U256::from(0u8),
            Default::default(),
            Arc::new(MemoryDBRepository::new()),
        )
        .unwrap();
        assert_eq!(state.code(&a).unwrap(), Some(Arc::new(vec![1u8, 2, 3])));
    }

    #[test]
    fn storage_at_from_database() {
        let a = Address::zero();
        let (root, db) = {
            let mut state = get_temp_state_with_nonce();
            state
                .set_storage(
                    &a,
                    H128::from(U128::from(2u64)),
                    H128::from(U128::from(69u64)),
                )
                .unwrap();
            state.commit().unwrap();
            state.drop()
        };

        let s = State::from_existing(
            db,
            root,
            U256::from(0u8),
            Default::default(),
            Arc::new(MemoryDBRepository::new()),
        )
        .unwrap();
        assert_eq!(
            s.storage_at(&a, &H128::from(U128::from(2u64))).unwrap(),
            H128::from(U128::from(69u64))
        );
    }

    #[test]
    fn get_from_database() {
        let a = Address::zero();
        let (root, db) = {
            let mut state = get_temp_state();
            state.inc_nonce(&a).unwrap();
            state
                .add_balance(&a, &U256::from(69u64), CleanupMode::NoEmpty)
                .unwrap();
            state.commit().unwrap();
            assert_eq!(state.balance(&a).unwrap(), U256::from(69u64));
            state.drop()
        };

        let state = State::from_existing(
            db,
            root,
            U256::from(1u8),
            Default::default(),
            Arc::new(MemoryDBRepository::new()),
        )
        .unwrap();
        assert_eq!(state.balance(&a).unwrap(), U256::from(69u64));
        assert_eq!(state.nonce(&a).unwrap(), U256::from(1u64));
    }

    #[test]
    fn remove() {
        let a = Address::zero();
        let mut state = get_temp_state();
        assert_eq!(state.exists(&a).unwrap(), false);
        assert_eq!(state.exists_and_not_null(&a).unwrap(), false);
        state.inc_nonce(&a).unwrap();
        assert_eq!(state.exists(&a).unwrap(), true);
        assert_eq!(state.exists_and_not_null(&a).unwrap(), true);
        assert_eq!(state.nonce(&a).unwrap(), U256::from(1u64));
        state.kill_account(&a);
        assert_eq!(state.exists(&a).unwrap(), false);
        assert_eq!(state.exists_and_not_null(&a).unwrap(), false);
        assert_eq!(state.nonce(&a).unwrap(), U256::from(0u64));
    }

    #[test]
    fn empty_account_is_not_created() {
        let a = Address::zero();
        let db = get_temp_state_db();
        let (root, db) = {
            let mut state = State::new(
                db,
                U256::from(0),
                Default::default(),
                Arc::new(MemoryDBRepository::new()),
            );
            state
                .add_balance(&a, &U256::default(), CleanupMode::NoEmpty)
                .unwrap(); // create an empty account
            state.commit().unwrap();
            state.drop()
        };
        let state = State::from_existing(
            db,
            root,
            U256::from(0u8),
            Default::default(),
            Arc::new(MemoryDBRepository::new()),
        )
        .unwrap();
        assert!(!state.exists(&a).unwrap());
        assert!(!state.exists_and_not_null(&a).unwrap());
    }

    #[test]
    fn empty_account_exists_when_creation_forced() {
        let a = Address::zero();
        let db = get_temp_state_db();
        let (root, db) = {
            println!("default balance = {}", U256::default());
            let mut state = State::new(
                db,
                U256::from(0),
                Default::default(),
                Arc::new(MemoryDBRepository::new()),
            );
            state
                .add_balance(&a, &U256::default(), CleanupMode::ForceCreate)
                .unwrap(); // create an empty account
            state.commit().unwrap();
            state.drop()
        };
        let state = State::from_existing(
            db,
            root,
            U256::from(0u8),
            Default::default(),
            Arc::new(MemoryDBRepository::new()),
        )
        .unwrap();

        assert!(!state.exists(&a).unwrap());
        assert!(!state.exists_and_not_null(&a).unwrap());
    }

    #[test]
    fn remove_from_database() {
        let a = Address::zero();
        let (root, db) = {
            let mut state = get_temp_state();
            state.inc_nonce(&a).unwrap();
            state.commit().unwrap();
            assert_eq!(state.exists(&a).unwrap(), true);
            assert_eq!(state.nonce(&a).unwrap(), U256::from(1u64));
            state.drop()
        };

        let (root, db) = {
            let mut state = State::from_existing(
                db,
                root,
                U256::from(0u8),
                Default::default(),
                Arc::new(MemoryDBRepository::new()),
            )
            .unwrap();
            assert_eq!(state.exists(&a).unwrap(), true);
            assert_eq!(state.nonce(&a).unwrap(), U256::from(1u64));
            state.kill_account(&a);
            state.commit().unwrap();
            assert_eq!(state.exists(&a).unwrap(), false);
            assert_eq!(state.nonce(&a).unwrap(), U256::from(0u64));
            state.drop()
        };

        let state = State::from_existing(
            db,
            root,
            U256::from(0u8),
            Default::default(),
            Arc::new(MemoryDBRepository::new()),
        )
        .unwrap();
        assert_eq!(state.exists(&a).unwrap(), false);
        assert_eq!(state.nonce(&a).unwrap(), U256::from(0u64));
    }

    #[test]
    fn alter_balance() {
        let mut state = get_temp_state();
        let a = Address::zero();
        let b = 1u64.into();
        state
            .add_balance(&a, &U256::from(69u64), CleanupMode::NoEmpty)
            .unwrap();
        assert_eq!(state.balance(&a).unwrap(), U256::from(69u64));
        state.commit().unwrap();
        assert_eq!(state.balance(&a).unwrap(), U256::from(69u64));
        state
            .sub_balance(&a, &U256::from(42u64), &mut CleanupMode::NoEmpty)
            .unwrap();
        assert_eq!(state.balance(&a).unwrap(), U256::from(27u64));
        state.commit().unwrap();
        assert_eq!(state.balance(&a).unwrap(), U256::from(27u64));
        state
            .transfer_balance(&a, &b, &U256::from(18u64), CleanupMode::NoEmpty)
            .unwrap();
        assert_eq!(state.balance(&a).unwrap(), U256::from(9u64));
        assert_eq!(state.balance(&b).unwrap(), U256::from(18u64));
        state.commit().unwrap();
        assert_eq!(state.balance(&a).unwrap(), U256::from(9u64));
        assert_eq!(state.balance(&b).unwrap(), U256::from(18u64));
    }

    #[test]
    fn alter_nonce() {
        let mut state = get_temp_state();
        let a = Address::zero();
        state.inc_nonce(&a).unwrap();
        assert_eq!(state.nonce(&a).unwrap(), U256::from(1u64));
        state.inc_nonce(&a).unwrap();
        assert_eq!(state.nonce(&a).unwrap(), U256::from(2u64));
        state.commit().unwrap();
        assert_eq!(state.nonce(&a).unwrap(), U256::from(2u64));
        state.inc_nonce(&a).unwrap();
        assert_eq!(state.nonce(&a).unwrap(), U256::from(3u64));
        state.commit().unwrap();
        assert_eq!(state.nonce(&a).unwrap(), U256::from(3u64));
    }

    #[test]
    fn balance_nonce() {
        let mut state = get_temp_state();
        let a = Address::zero();
        assert_eq!(state.balance(&a).unwrap(), U256::from(0u64));
        assert_eq!(state.nonce(&a).unwrap(), U256::from(0u64));
        state.commit().unwrap();
        assert_eq!(state.balance(&a).unwrap(), U256::from(0u64));
        assert_eq!(state.nonce(&a).unwrap(), U256::from(0u64));
    }

    #[test]
    fn ensure_cached() {
        let mut state = get_temp_state_with_nonce();
        let a = Address::zero();
        state.require(&a, false).unwrap();
        state.commit().unwrap();
        assert_eq!(
            *state.root(),
            "9d6d4b335038e1ffe0f060c29e52d6eed2aec4a085dfa37afba9d1e10cc7be85".into()
        );
    }

    #[test]
    fn checkpoint_basic() {
        let mut state = get_temp_state();
        let a = Address::zero();
        state.checkpoint();
        state
            .add_balance(&a, &U256::from(69u64), CleanupMode::NoEmpty)
            .unwrap();
        assert_eq!(state.balance(&a).unwrap(), U256::from(69u64));
        state.discard_checkpoint();
        assert_eq!(state.balance(&a).unwrap(), U256::from(69u64));
        state.checkpoint();
        state
            .add_balance(&a, &U256::from(1u64), CleanupMode::NoEmpty)
            .unwrap();
        assert_eq!(state.balance(&a).unwrap(), U256::from(70u64));
        state.revert_to_checkpoint();
        assert_eq!(state.balance(&a).unwrap(), U256::from(69u64));
    }

    #[test]
    fn checkpoint_nested() {
        let mut state = get_temp_state();
        let a = Address::zero();
        state.checkpoint();
        state.checkpoint();
        state
            .add_balance(&a, &U256::from(69u64), CleanupMode::NoEmpty)
            .unwrap();
        assert_eq!(state.balance(&a).unwrap(), U256::from(69u64));
        state.discard_checkpoint();
        assert_eq!(state.balance(&a).unwrap(), U256::from(69u64));
        state.revert_to_checkpoint();
        assert_eq!(state.balance(&a).unwrap(), U256::from(0));
    }

    #[test]
    fn create_empty() {
        let mut state = get_temp_state();
        state.commit().unwrap();
        assert_eq!(
            *state.root(),
            "45b0cfc220ceec5b7c1c62c4d4193d38e4eba48e8815729ce75f9c0ab0e4c1c0".into()
        );
    }

    #[test]
    fn should_not_panic_on_state_diff_with_storage() {
        let mut state = get_temp_state();

        let a: Address = 0xa.into();
        state.init_code(&a, b"abcdefg".to_vec()).unwrap();;
        state
            .add_balance(&a, &256.into(), CleanupMode::NoEmpty)
            .unwrap();
        state.set_storage(&a, 0xb.into(), 0xc.into()).unwrap();

        let mut new_state = state.clone();
        new_state.set_storage(&a, 0xb.into(), 0xd.into()).unwrap();

        new_state.diff_from(state).unwrap();
    }
}
