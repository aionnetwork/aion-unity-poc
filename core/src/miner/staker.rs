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

use aion_types::{Address, U256};
use client::{BlockChainClient};

use key::{Ed25519KeyPair, Ed25519Secret, public_to_address_ed25519};

pub struct Staker {
    address: Address,
    sk: [u8; 64],
}


impl Staker {
    /// Create a staking client using a private key
    pub fn new(sk: [u8; 64]) -> Staker {
        let s = Ed25519Secret::from_slice(&sk).expect("Invalid private key");
        let key = Ed25519KeyPair::from_secret(s).expect("Failed to compute public key");
        Staker {
            address: public_to_address_ed25519(key.public()),
            sk,
        }
    }

    /// Query the time delay of the staking account
    pub fn time_delay(&self, client: &BlockChainClient) -> U256 {
        U256::from(10)
    }

    /// Produce a PoS block
    pub fn produce_block(&self, client: &BlockChainClient) {

    }
}