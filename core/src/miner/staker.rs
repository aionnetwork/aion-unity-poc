/*******************************************************************************
 * Copyright (c) 2019 Aion foundation.
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

use std::sync::Arc;
use std::cmp::max;

use tiny_keccak::Keccak;

use aion_types::{Address, H128, U128, U512};
use blake2b::blake2b;
use block::IsBlock;
use client::{BlockId, MiningBlockChainClient};
use engines::EthEngine;
use rcrypto::ed25519::{keypair, signature};
use spec::Spec;
use header::SealType;

use super::Miner;
use ansi_term::Colour;

/*
===========================
ED25519 basics
===========================
public key = 32 bytes
private key = 32 bytes
signature = 64 bytes
signature (with public key) = 96 bytes
*/

/// Represents a staking client
pub struct Staker {
    engine: Arc<EthEngine>,
    staking_registry: Address,
    address: Address,
    keypair: [u8; 64], // private key + public key
}

/// Errors encountered when submitting a PoS block
pub enum Error {
    /// The seed + signature are invalid.
    PosInvalid,
    /// Failed to import the block
    FailedToImport,
}

impl Staker {
    /// Create a staking client using a private key
    pub fn new(spec: &Spec, staking_registry: Address, sk: [u8; 32]) -> Staker {
        let (keypair, pk) = keypair(&sk);

        let hash = blake2b(pk);
        let mut address = Address::default();
        address.copy_from_slice(&hash[..]);
        address.0[0] = 0xA0;

        Staker {
            engine: spec.engine.clone(),
            staking_registry,
            address,
            keypair,
        }
    }

    /// Calculate the block producing time of this staker
    pub fn calc_produce_time(&self, client: &MiningBlockChainClient) -> u64 {
        let map_offset: [u8; 32] = [
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x03,
        ];
        let map_key = self.address.0;

        let mut storage_key: [u8; 32] = [0; 32];
        let mut sha3 = Keccak::new_sha3_256();
        sha3.update(&map_offset);
        sha3.update(&map_key);
        sha3.finalize(&mut storage_key);

        let stake = client
            .storage_at(
                &self.staking_registry,
                &H128::from(&storage_key[0..16]),
                BlockId::Latest,
            )
            .unwrap_or(H128::default());
        let mut stake = U128::from(stake).as_u64();
        // TODO: remove the following line
        stake = 16u64;
        if stake == 0 {
            return 0xffffffffffffffffu64;
        }

        // timestamp and previous seed
        let parent_header = client.best_block_header_with_seal_type(&SealType::Pos);
        let (timestamp, seed) = match parent_header.clone() {
            Some(parent) => {
                let seal = parent.seal();
                let seed = seal.get(0).expect("A pos block has to contain a seeds");
                (parent.timestamp(), seed.clone())
            },
            None => (0u64, Vec::new()),
        };

        // difficulty
        let grand_parent_header =  match parent_header.clone() {
            Some(parent) => client.previous_block_header_with_seal_type(&parent.hash(), &SealType::Pos),
            None => None,
        };
        let difficulty = client.calculate_difficulty(&parent_header, &grand_parent_header);

        // \Delta = \frac{d_s \cdot ln({2^{256}}/{hash(seed)})}{V}.
        // NOTE: never use floating point in production
        let new_seed = self.sign(&seed);
        let hash_of_seed = blake2b(&new_seed[..]);
        let u = (U512::from(1) << 256) / U512::from(&hash_of_seed[..]);
        let delta = (difficulty.as_u64() as f64)
            * (u.as_u64() as f64).ln()
            / (stake as f64);
        trace!(target: "staker", "Staking...difficulty: {}, u: {}, stake: {} delta: {}", difficulty.as_u64(), u, stake, delta);

        timestamp + max(1u64, delta as u64)
    }

    /// Produce a PoS block
    pub fn produce_block(
        &self,
        miner: &Miner,
        client: &MiningBlockChainClient,
    ) -> Result<(), Error>
    {
        // 1. create a PoS block template
        let (raw_block, _) = miner.prepare_block(client, Some(&SealType::Pos));
        let parent_hash = raw_block.header().parent_hash().clone();
        let bare_hash = raw_block.header().bare_hash();

        // 2. compute the seed and signature
        let latest_pos_block_header =
            client.latest_block_header_with_seal_type(&parent_hash, &SealType::Pos);
        let latest_seed = match latest_pos_block_header {
            Some(header) => {
                let seal = header.seal();
                let seed = seal.get(0).expect("A pos block has to contain a seeds");
                seed.clone()
            }
            None => Vec::new(),
        };

        let seed = self.sign(&latest_seed);
        let signature = self.sign(&bare_hash.0);

        // 3. seal the block
        let mut seal: Vec<Vec<u8>> = Vec::new();
        seal.push(seed.to_vec());
        seal.push(signature.to_vec());
        let sealed_block = raw_block
            .lock()
            .try_seal(&*self.engine, seal)
            .or_else(|(e, _)| {
                warn!(target: "staker", "Seed + signature rejected: {}", e);
                Err(Error::PosInvalid)
            })?;

        // 4. import the block
        let n = sealed_block.header().number();
        let d = sealed_block.header().difficulty().clone();
        let h = sealed_block.header().hash();
        client.import_sealed_block(sealed_block).or_else(|e| {
            warn!(target: "staker", "Failed to import: {}", e);
            Err(Error::FailedToImport)
        })?;

        // 5. done!
        info!(target: "miner", "PoS block imported OK. #{}: {}, {}",
              Colour::White.bold().paint(format!("{}", n)),
              Colour::White.bold().paint(format!("{}", d)),
              Colour::White.bold().paint(format!("{:x}", h)));
        Ok(())
    }

    fn sign(&self, message: &[u8]) -> [u8; 96] {
        let pk = &self.keypair[32..64];
        let signature = signature(message, &self.keypair);

        let mut result = [0u8; 96];
        result[0..32].copy_from_slice(pk);
        result[32..96].copy_from_slice(&signature);

        result
    }
}
