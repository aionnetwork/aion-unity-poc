/*******************************************************************************
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

use std::cmp;
use super::DifficultyCalc;
use error::{Error,BlockError};
use header::Header;
use unexpected::{Mismatch};
use key::public_to_address_ed25519;
use rcrypto::ed25519::verify;
use tiny_keccak::Keccak;
use state::State;
use state_db::StateDB;
use aion_types::{Address, H128, U128, H256};
use blake2b::blake2b;
use rustc_hex::FromHex;
use num_bigint::BigUint;

pub trait GrantParentHeaderValidator {
    fn validate(
        &self,
        header: &Header,
        parent_header: Option<&Header>,
        grant_parent_header: Option<&Header>,
        state: Option<State<StateDB>>,
    ) -> Result<(), Error>;
}

pub struct DifficultyValidator<'a> {
    pub difficulty_calc: &'a DifficultyCalc,
}

impl<'a> GrantParentHeaderValidator for DifficultyValidator<'a> {
    fn validate(
        &self,
        header: &Header,
        parent_header: Option<&Header>,
        grant_parent_header: Option<&Header>,
        _state: Option<State<StateDB>>,
    ) -> Result<(), Error>
    {
        let difficulty = header.difficulty().clone();
        let calc_difficulty = self
            .difficulty_calc
            .calculate_difficulty_v1(parent_header, grant_parent_header);
        if difficulty != calc_difficulty {
            Err(BlockError::InvalidDifficulty(Mismatch {
                expected: calc_difficulty,
                found: difficulty,
            })
            .into())
        } else {
            Ok(())
        }
    }
}

pub struct POSValidator;

impl GrantParentHeaderValidator for POSValidator {
    fn validate(
        &self,
        header: &Header,
        parent_header: Option<&Header>,
        _grant_parent_header: Option<&Header>,
        state: Option<State<StateDB>>,
    ) -> Result<(), Error>
    {
        // First pos block, skip the check
        if parent_header.is_none() {
            return Ok(()); // This is problematic in production
        }
        let parent_header = parent_header.expect("Parent block header unwrap tested before.");
        let seal = header.seal();
        if seal.len() != 2 {
            error!(target: "pos", "seal length != 2");
            return Err(BlockError::InvalidSealArity(Mismatch {
                expected: 2,
                found: seal.len(),
            })
            .into());
        }

        let seed = &seal[0];
        let signature = &seal[1];
        let difficulty = header.difficulty().clone();
        let timestamp = header.timestamp();
        let parent_seed = parent_header
            .seal()
            .get(0)
            .expect("parent pos block should have a seed");
        let parent_timestamp = parent_header.timestamp();

        // Verify seed
        let public_from_seed = &seed[..32];
        let sig_from_seed = &seed[32..96];
        if !verify(&parent_seed, public_from_seed, sig_from_seed) {
            return Err(BlockError::InvalidSeal.into());
        }
        let sender_from_seed = public_to_address_ed25519(&H256::from(public_from_seed));

        // Verify block signature
        let public_from_block = &signature[..32];
        let sig_from_block = &signature[32..96];
        if !verify(&header.bare_hash().0, public_from_block, sig_from_block) {
            return Err(BlockError::InvalidSeal.into());
        }
        let sender_from_block = public_to_address_ed25519(&H256::from(public_from_block));

        // Verify seed and block signature came from the same author
        if sender_from_seed != sender_from_block {
            return Err(BlockError::InvalidSeal.into());
        }

        let state = state.expect("State should exist.");
        // Verify block timestamp
        let stake = self.calculate_stake(sender_from_seed, state);
        let hash_of_seed = blake2b(&seed[..]);
        let a = BigUint::parse_bytes(b"ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff", 16).unwrap();
        let b = BigUint::from_bytes_be(&hash_of_seed[..]);
        let u = POSValidator::ln(&a).unwrap() - POSValidator::ln(&b).unwrap();
        let delta = match stake {
            0 => 1_000_000_000_000f64,
            _ => (difficulty.as_u64() as f64) * u / (stake as f64),
        };
        let delta_int = cmp::max(1u64, delta as u64);
        trace!(target: "pos", "pos block time validation. block timestamp: {}, parent timestamp: {}, expected delta: {}", timestamp, parent_timestamp, delta_int);
        if timestamp - parent_timestamp < delta_int {
            return Err(
                BlockError::InvalidPosTimestamp(timestamp, parent_timestamp, delta_int).into(),
            );
        }
        Ok(())
    }
}

impl POSValidator {

    // Credit: https://www.reddit.com/r/rust/comments/6gxvs2/big_numbers_in_rust/
    fn ln(x: &BigUint) -> Result<f64, String> {
        let x: Vec<u8> = x.to_bytes_le();

        const BYTES: usize = 12;
        let start = if x.len() < BYTES {
            0
        } else {
            x.len() - BYTES
        };

        let mut n: f64 = 0.0;
        for i in start..x.len() {
            n = n / 256f64 + (x[i] as f64);
        }
        let ln_256: f64 = (256f64).ln();

        Ok(n.ln() + ln_256 * ((x.len() - 1) as f64))
    }

    fn calculate_stake(&self, address: Address, state: State<StateDB>) -> u64 {
        let staking_registry = Address::from_slice(
            "a00876be75b664de079b58e7acbf70ce315ba4aaa487f7ddf2abd5e0e1a8dff4"
                .from_hex()
                .unwrap()
                .as_slice(),
        );

        let map_key = address.0;

        let map_offset: [u8; 16] = [
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x06,
        ];

        let mut storage_key: [u8; 32] = [0; 32];
        let mut digest = Keccak::new_keccak256();
        digest.update(&map_key);
        digest.update(&map_offset);
        digest.finalize(&mut storage_key);

        let stake = state
            .storage_at(&staking_registry, &H128::from(&storage_key[0..16]))
            .unwrap_or(H128::default());
        U128::from(stake).as_u64()
    }
}
