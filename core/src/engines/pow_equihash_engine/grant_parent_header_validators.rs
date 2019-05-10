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

use super::DifficultyCalc;
use error::{Error,BlockError};
use header::Header;
use unexpected::{Mismatch};
use key::{recover_ed25519, public_to_address_ed25519};
use bytes::to_hex;

pub trait GrantParentHeaderValidator {
    fn validate(
        &self,
        header: &Header,
        parent_header: &Header,
        grant_parent_header: Option<&Header>,
    ) -> Result<(), Error>;
}

pub struct DifficultyValidator<'a> {
    pub difficulty_calc: &'a DifficultyCalc,
}

impl<'a> GrantParentHeaderValidator for DifficultyValidator<'a> {
    fn validate(
        &self,
        header: &Header,
        parent_header: &Header,
        grant_parent_header: Option<&Header>,
    ) -> Result<(), Error>
    {
        let difficulty = *header.difficulty();
        let parent_difficulty = *parent_header.difficulty();
        if parent_header.number() == 0u64 {
            if difficulty != parent_difficulty {
                return Err(BlockError::InvalidDifficulty(Mismatch {
                    expected: parent_difficulty,
                    found: difficulty,
                })
                .into());
            } else {
                return Ok(());
            }
        }

        if grant_parent_header.is_none() {
            panic!(
                "non-1st block must have grant parent. block num: {}",
                header.number()
            );
        } else {
            let calc_difficulty = self
                .difficulty_calc
                .calculate_difficulty_v1(Some(parent_header), grant_parent_header);
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
}

pub struct POSValidator;

impl GrantParentHeaderValidator for POSValidator {
    fn validate(
        &self,
        header: &Header,
        parent_header: &Header,
        _grant_parent_header: Option<&Header>,
    ) -> Result<(), Error>
    {
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
        debug!(target: "pos", "seed: {}", to_hex(seed.as_slice()));
        let signature = &seal[1];
        debug!(target: "pos", "signature: {}", to_hex(signature.as_slice()));

        let parent_seed = parent_header
            .seal()
            .get(0)
            .expect("parent pos block should have a seed");

        let public = recover_ed25519(&seed.clone().into(), &parent_seed.as_slice().into())?;
        let _sender = public_to_address_ed25519(&public);

        Ok(())
    }
}
