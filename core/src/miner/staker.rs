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

use aion_types::{Address, U256, Ed25519Public};
use client::MiningBlockChainClient;
use key::{Ed25519KeyPair, Ed25519Secret, public_to_address_ed25519, sign_ed25519, H256};
use super::Miner;
use block::IsBlock;
use spec::Spec;
use engines::EthEngine;
use std::sync::Arc;

pub struct Staker {
    engine: Arc<EthEngine>,
    staking_registry: Address,
    address: Address,
    sk: Ed25519Secret,
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
        sk: [u8; 64],
    ) -> Staker {
        let s = Ed25519Secret::from_slice(&sk).expect("Invalid private key");
        let key = Ed25519KeyPair::from_secret(s).expect("Failed to compute public key");
        Staker {
            engine: spec.engine.clone(),
            staking_registry,
            address: public_to_address_ed25519(key.public()),
            sk: key.secret().clone(),
        }
    }

    /// Query the time delay of the staking account
    pub fn time_delay(&self, client: &MiningBlockChainClient) -> U256 {
        U256::from(10)
    }

    /// Produce a PoS block
    pub fn produce_block(&self, miner: &Miner, client: &MiningBlockChainClient) -> Result<(), Error> {
        // 0. get the latest pos block
        let latest_pos_block = client.latest_pos_block();
        let latest_seed = match latest_pos_block {
            Some(b) => {
                let seal = b.header().seal();
                let seed = seal.get(0).expect("A pos block has to contain a seeds");
                H256::from(&seed[0..96])
            }
            None => H256::zero(),
        };

        // 1. compute the new seed
        let seed = H256::zero();
        let seed = sign_ed25519(&self.sk, &latest_seed)
            .expect("Failed to sign the previous seed");

        // 2. create and sign a block
        let (raw_block, _) = miner.prepare_block(client);
        let bare_hash = raw_block.header().bare_hash();
        let signature = sign_ed25519(&self.sk, &bare_hash)
            .expect("Failed to sign a block");

        // 3. seal the block
        let mut seal: Vec<Vec<u8>> = Vec::new();
        seal.push(seed.get_signature().clone().into());
        seal.push(signature.get_signature().clone().into());
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
}