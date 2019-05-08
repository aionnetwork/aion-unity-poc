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

use aion_types::Address;
use block::IsBlock;
use client::MiningBlockChainClient;
use engines::EthEngine;
use blake2b::blake2b;
use rcrypto::ed25519::{keypair, signature};
use spec::Spec;

use super::Miner;

/*
===========================
ED25519 basics
===========================
public key = 32 bytes
private key = 32 bytes
signature = 64 bytes
signature (with public key) = 96 bytes
*/

pub struct Staker {
    engine: Arc<EthEngine>,
    staking_registry: Address,
    address: Address,
    keypair: [u8; 64], // private key + public key
}

pub enum Error {
    /// The seed + signature are invalid.
    PosInvalid,
    /// Failed to import the block
    FailedToImport,
}


impl Staker {
    /// Create a staking client using a private key
    pub fn new(
        spec: &Spec,
        staking_registry: Address,
        sk: [u8; 32],
    ) -> Staker {
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
        0u64
    }

    /// Produce a PoS block
    pub fn produce_block(&self, miner: &Miner, client: &MiningBlockChainClient) -> Result<(), Error> {
        // 0. get the latest pos block
        let latest_pos_block = client.latest_pos_block();
        let latest_seed = match latest_pos_block {
            Some(b) => {
                let seal = b.header().seal();
                let seed = seal.get(0).expect("A pos block has to contain a seeds");
                seed.clone()
            }
            None => Vec::new(),
        };

        // 1. compute the new seed
        let seed = self.sign(&latest_seed);

        // 2. create and sign a block
        let (raw_block, _) = miner.prepare_block(client);
        let bare_hash = raw_block.header().bare_hash();
        let signature = self.sign(&bare_hash.0);

        // 3. seal the block
        let mut seal: Vec<Vec<u8>> = Vec::new();
        seal.push(seed.to_vec());
        seal.push(signature.to_vec());
        let sealed_block = raw_block.lock().try_seal(&*self.engine, seal).or_else(|(e, _)| {
            warn!(target: "staker", "Seed + signature rejected: {}", e);
            Err(Error::PosInvalid)
        })?;

        // 4. import the block
        client.import_sealed_block(sealed_block).or_else(|e| {
            warn!(target: "staker", "Failed to import: {}", e);
            Err(Error::FailedToImport)
        })?;

        // 5. done!
        info!(target: "staker", "The PoS block was imported.");
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