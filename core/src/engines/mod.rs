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

//! Consensus engine specification and basic implementations.

mod instant_seal;
mod null_engine;
pub mod pow_equihash_engine;

pub mod epoch;

pub use self::epoch::{EpochVerifier, Transition as EpochTransition};
pub use self::instant_seal::InstantSeal;
pub use self::null_engine::NullEngine;
pub use self::pow_equihash_engine::POWEquihashEngine;

use std::sync::{Weak, Arc};
use std::collections::{BTreeMap, HashMap};
use std::fmt;

use self::epoch::PendingTransition;

use precompiled::builtin::BuiltinContract;
use error::Error;
use header::{Header, BlockNumber};
use spec::CommonParams;
use transaction::{UnverifiedTransaction, SignedTransaction};

use aion_machine::{Machine, LocalizedMachine as Localized};
use aion_types::{H256, U256, Address};
use unexpected::{Mismatch, OutOfBounds};
use bytes::Bytes;
use state::State;
use state_db::StateDB;

/// Default EIP-210 contrat code.
/// As defined in https://github.com/ethereum/EIPs/pull/210
pub const DEFAULT_BLOCKHASH_CONTRACT: &'static str = "73fffffffffffffffffffffffffffffffffffffffe33141561006a5760014303600035610100820755610100810715156100455760003561010061010083050761010001555b6201000081071515610064576000356101006201000083050761020001555b5061013e565b4360003512151561008457600060405260206040f361013d565b61010060003543031315156100a857610100600035075460605260206060f361013c565b6101006000350715156100c55762010000600035430313156100c8565b60005b156100ea576101006101006000350507610100015460805260206080f361013b565b620100006000350715156101095763010000006000354303131561010c565b60005b1561012f57610100620100006000350507610200015460a052602060a0f361013a565b600060c052602060c0f35b5b5b5b5b";

/// Voting errors.
#[derive(Debug)]
pub enum EngineError {
    /// Signature or author field does not belong to an authority.
    NotAuthorized(Address),
    /// The same author issued different votes at the same step.
    DoubleVote(Address),
    /// The received block is from an incorrect proposer.
    NotProposer(Mismatch<Address>),
    /// Message was not expected.
    UnexpectedMessage,
    /// Seal field has an unexpected size.
    BadSealFieldSize(OutOfBounds<usize>),
    /// Validation proof insufficient.
    InsufficientProof(String),
    /// Failed system call.
    FailedSystemCall(String),
    /// Malformed consensus message.
    MalformedMessage(String),
    /// Requires client ref, but none registered.
    RequiresClient,
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::EngineError::*;
        let msg = match *self {
            DoubleVote(ref address) => format!("Author {} issued too many blocks.", address),
            NotProposer(ref mis) => format!("Author is not a current proposer: {}", mis),
            NotAuthorized(ref address) => format!("Signer {} is not authorized.", address),
            UnexpectedMessage => "This Engine should not be fed messages.".into(),
            BadSealFieldSize(ref oob) => format!("Seal field has an unexpected length: {}", oob),
            InsufficientProof(ref msg) => format!("Insufficient validation proof: {}", msg),
            FailedSystemCall(ref msg) => format!("Failed to make system call: {}", msg),
            MalformedMessage(ref msg) => format!("Received malformed consensus message: {}", msg),
            RequiresClient => format!("Call requires client but none registered"),
        };

        f.write_fmt(format_args!("Engine error ({})", msg))
    }
}

/// Seal type.
#[derive(Debug, PartialEq, Eq)]
pub enum Seal {
    /// Proposal seal; should be broadcasted, but not inserted into blockchain.
    Proposal(Vec<Bytes>),
    /// Regular block seal; should be part of the blockchain.
    Regular(Vec<Bytes>),
    /// Engine does generate seal for this block right now.
    None,
}

/// Type alias for a function we can get headers by hash through.
pub type Headers<'a, H> = Fn(H256) -> Option<H> + 'a;

/// Type alias for a function we can query pending transitions by block hash through.
pub type PendingTransitionStore<'a> = Fn(H256) -> Option<PendingTransition> + 'a;

/// Proof dependent on state.
pub trait StateDependentProof<M: Machine>: Send + Sync {
    /// Generate a proof, given the state.
    // TODO: make this into an &M::StateContext
    fn generate_proof<'a>(
        &self,
        state: &<M as Localized<'a>>::StateContext,
    ) -> Result<Vec<u8>, String>;
    /// Check a proof generated elsewhere (potentially by a peer).
    // `engine` needed to check state proofs, while really this should
    // just be state machine params.
    fn check_proof(&self, machine: &M, proof: &[u8]) -> Result<(), String>;
}

/// Proof generated on epoch change.
pub enum Proof<M: Machine> {
    /// Known proof (extracted from signal)
    Known(Vec<u8>),
    /// State dependent proof.
    WithState(Arc<StateDependentProof<M>>),
}

/// Generated epoch verifier.
pub enum ConstructedVerifier<'a, M: Machine> {
    /// Fully trusted verifier.
    Trusted(Box<EpochVerifier<M>>),
    /// Verifier unconfirmed. Check whether given finality proof finalizes given hash
    /// under previous epoch.
    Unconfirmed(Box<EpochVerifier<M>>, &'a [u8], H256),
    /// Error constructing verifier.
    Err(Error),
}

impl<'a, M: Machine> ConstructedVerifier<'a, M> {
    /// Convert to a result, indicating that any necessary confirmation has been done
    /// already.
    pub fn known_confirmed(self) -> Result<Box<EpochVerifier<M>>, Error> {
        match self {
            ConstructedVerifier::Trusted(v) | ConstructedVerifier::Unconfirmed(v, _, _) => Ok(v),
            ConstructedVerifier::Err(e) => Err(e),
        }
    }
}

/// Results of a query of whether an epoch change occurred at the given block.
pub enum EpochChange<M: Machine> {
    /// Cannot determine until more data is passed.
    Unsure(M::AuxiliaryRequest),
    /// No epoch change.
    No,
    /// The epoch will change, with proof.
    Yes(Proof<M>),
}

/// A consensus mechanism for the chain. Generally either proof-of-work or proof-of-stake-based.
/// Provides hooks into each of the major parts of block import.
pub trait Engine<M: Machine>: Sync + Send {
    /// The name of this engine.
    fn name(&self) -> &str;

    /// Get access to the underlying state machine.
    // TODO: decouple.
    fn machine(&self) -> &M;

    /// The number of additional header fields required for this engine.
    fn seal_fields(&self, _header: &M::Header) -> usize { 0 }

    /// Additional engine-specific information for the user/developer concerning `header`.
    fn extra_info(&self, _header: &M::Header) -> BTreeMap<String, String> { BTreeMap::new() }

    /// Block transformation functions, before the transactions.
    /// `epoch_begin` set to true if this block kicks off an epoch.
    fn on_new_block(&self, _block: &mut M::LiveBlock, _epoch_begin: bool) -> Result<(), M::Error> {
        Ok(())
    }

    /// Block transformation functions, after the transactions.
    fn on_close_block(&self, _block: &mut M::LiveBlock) -> Result<(), M::Error> { Ok(()) }

    /// None means that it requires external input (e.g. PoW) to seal a block.
    /// Some(true) means the engine is currently prime for seal generation (i.e. node is the current validator).
    /// Some(false) means that the node might seal internally but is not qualified now.
    fn seals_internally(&self) -> Option<bool> { None }

    /// Attempt to seal the block internally.
    ///
    /// If `Some` is returned, then you get a valid seal.
    ///
    /// This operation is synchronous and may (quite reasonably) not be available, in which None will
    /// be returned.
    ///
    /// It is fine to require access to state or a full client for this function, since
    /// light clients do not generate seals.
    fn generate_seal(&self, _block: &M::LiveBlock, _parent: &M::Header) -> Seal { Seal::None }

    /// Verify a locally-generated seal of a header.
    ///
    /// If this engine seals internally,
    /// no checks have to be done here, since all internally generated seals
    /// should be valid.
    ///
    /// Externally-generated seals (e.g. PoW) will need to be checked for validity.
    ///
    /// It is fine to require access to state or a full client for this function, since
    /// light clients do not generate seals.
    fn verify_local_seal(&self, header: &M::Header) -> Result<(), M::Error>;

    /// Phase 1 quick block verification. Only does checks that are cheap. Returns either a null `Ok` or a general error detailing the problem with import.
    fn verify_block_basic(&self, _header: &M::Header) -> Result<(), M::Error> { Ok(()) }

    /// Phase 2 verification. Perform costly checks such as transaction signatures. Returns either a null `Ok` or a general error detailing the problem with import.
    fn verify_block_unordered(&self, _header: &M::Header) -> Result<(), M::Error> { Ok(()) }

    /// Phase 3 verification. Check block information against parent. Returns either a null `Ok` or a general error detailing the problem with import.
    fn verify_block_family(
        &self,
        _header: &M::Header,
        _parent: &M::Header,
        _seal_parent: Option<&M::Header>,
        _seal_grant_parent: Option<&M::Header>,
        _state: Option<State<StateDB>>,
    ) -> Result<(), Error>
    {
        Ok(())
    }

    /// Phase 4 verification. Verify block header against potentially external data.
    /// Should only be called when `register_client` has been called previously.
    fn verify_block_external(&self, _header: &M::Header) -> Result<(), Error> { Ok(()) }

    /// Genesis epoch data.
    fn genesis_epoch_data<'a>(
        &self,
        _header: &M::Header,
        _state: &<M as Localized<'a>>::StateContext,
    ) -> Result<Vec<u8>, String>
    {
        Ok(Vec::new())
    }

    /// Whether an epoch change is signalled at the given header but will require finality.
    /// If a change can be enacted immediately then return `No` from this function but
    /// `Yes` from `is_epoch_end`.
    ///
    /// If auxiliary data of the block is required, return an auxiliary request and the function will be
    /// called again with them.
    /// Return `Yes` or `No` when the answer is definitively known.
    ///
    /// Should not interact with state.
    fn signals_epoch_end<'a>(
        &self,
        _header: &M::Header,
        _aux: <M as Localized<'a>>::AuxiliaryData,
    ) -> EpochChange<M>
    {
        EpochChange::No
    }

    /// Whether a block is the end of an epoch.
    ///
    /// This either means that an immediate transition occurs or a block signalling transition
    /// has reached finality. The `Headers` given are not guaranteed to return any blocks
    /// from any epoch other than the current.
    ///
    /// Return optional transition proof.
    fn is_epoch_end(
        &self,
        _chain_head: &M::Header,
        _chain: &Headers<M::Header>,
        _transition_store: &PendingTransitionStore,
    ) -> Option<Vec<u8>>
    {
        None
    }

    /// Create an epoch verifier from validation proof and a flag indicating
    /// whether finality is required.
    fn epoch_verifier<'a>(
        &self,
        _header: &M::Header,
        _proof: &'a [u8],
    ) -> ConstructedVerifier<'a, M>
    {
        ConstructedVerifier::Trusted(Box::new(self::epoch::NoOp))
    }

    /// Populate a header's fields based on its parent's header.
    /// Usually implements the chain scoring rule based on weight.
    fn populate_from_parent(
        &self,
        _header: &mut M::Header,
        _parent: Option<&M::Header>,
        _grant_parent: Option<&M::Header>,
    )
    {
    }

    /// Calculate difficulty
    fn calculate_difficulty(
        &self,
        _version: u8,
        _parent: Option<&Header>,
        _grand_parent: Option<&Header>,
    ) -> U256
    {
        U256::from(0)
    }

    /// Handle any potential consensus messages;
    /// updating consensus state and potentially issuing a new one.
    fn handle_message(&self, _message: &[u8]) -> Result<(), EngineError> {
        Err(EngineError::UnexpectedMessage)
    }

    /// Find out if the block is a proposal block and should not be inserted into the DB.
    /// Takes a header of a fully verified block.
    fn is_proposal(&self, _verified_header: &M::Header) -> bool { false }

    /// Add Client which can be used for sealing, potentially querying the state and sending messages.
    fn register_client(&self, _client: Weak<M::EngineClient>) {}

    //    /// Trigger next step of the consensus engine.
    //    fn step(&self) {}
    //
    /// Stops any services that the may hold the Engine and makes it safe to drop.
    fn stop(&self) {}
}

/// Common type alias for an engine coupled with an Ethereum-like state machine.
// TODO: make this a _trait_ alias when those exist.
// fortunately the effect is largely the same since engines are mostly used
// via trait objects.
pub trait EthEngine: Engine<::machine::EthereumMachine> {
    /// Get the general parameters of the chain.
    fn params(&self) -> &CommonParams { self.machine().params() }

    /// Builtin-contracts for the chain..
    fn builtins(&self) -> &BTreeMap<Address, Box<BuiltinContract>> { self.machine().builtins() }

    /// Attempt to get a handle to a built-in contract.
    /// Only returns references to activated built-ins.
    fn builtin(&self, a: &Address, block_number: BlockNumber) -> Option<&Box<BuiltinContract>> {
        self.machine().builtin(a, block_number)
    }

    /// Some intrinsic operation parameters; by default they take their value from the `spec()`'s `engine_params`.
    fn maximum_extra_data_size(&self) -> usize { self.machine().maximum_extra_data_size() }

    /// The nonce with which accounts begin at given block.
    fn account_start_nonce(&self, block: BlockNumber) -> U256 {
        self.machine().account_start_nonce(block)
    }

    /// Verify a transaction's signature is valid.
    fn verify_transaction_signature(
        &self,
        t: UnverifiedTransaction,
        header: &Header,
    ) -> Result<SignedTransaction, Error>
    {
        self.machine().verify_transaction_signature(t, header)
    }

    /// Additional verification for transactions in blocks.
    // TODO: Add flags for which bits of the transaction to check.
    // TODO: consider including State in the params.
    fn verify_transaction_basic(&self, t: &UnverifiedTransaction) -> Result<(), Error> {
        self.machine().verify_transaction_basic(t)
    }

    /// Additional information.
    fn additional_params(&self) -> HashMap<String, String> { self.machine().additional_params() }
}

// convenience wrappers for existing functions.
impl<T> EthEngine for T where T: Engine<::machine::EthereumMachine> {}
