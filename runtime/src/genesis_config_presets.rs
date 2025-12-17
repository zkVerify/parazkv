// Copyright 2024, Horizen Labs, Inc.
// Copyright (C) Parity Technologies (UK) Ltd.

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

#![allow(clippy::type_complexity)]

use crate::{currency::Balance, types::AccountId, SessionKeys, UNIT};
use alloc::{vec, vec::Vec};
use cumulus_primitives_core::ParaId;
use cumulus_primitives_core::relay_chain::AccountPublic;
use frame_support::__private::serde_json;
use parachains_common::AuraId;
use sp_core::{sr25519, Pair, Public};
use sp_genesis_builder::PresetId;
use sp_runtime::traits::IdentifyAccount;

const PARA_ID : u32 = 1599;

const ENDOWMENT: Balance = 1_000_000 * UNIT;
const DEFAULT_ENDOWED_SEEDS: [&str; 6] = ["Alice", "Bob", "Charlie", "Dave", "Eve", "Ferdie"];
const SAFE_XCM_VERSION: u32 = xcm::prelude::XCM_VERSION;

pub type Ids = (AccountId, AuraId);

/// Generate a crypto pair from seed.
pub fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
    TPublic::Pair::from_string(&alloc::format!("//{seed}"), None)
        .expect("static values are valid; qed")
        .public()
}

/// Generate an account ID from seed.
pub fn get_account_id_from_seed<TPublic: Public>(seed: &str) -> AccountId
where
    AccountPublic: From<<TPublic::Pair as Pair>::Public>,
{
    AccountPublic::from(get_from_seed::<TPublic>(seed)).into_account()
}

#[derive(Clone)]
pub struct FundedAccount {
    /// The account-id
    account_id: AccountId,
    /// Initial balance
    balance: Balance,
}

impl FundedAccount {
    pub const fn new(account_id: AccountId, balance: Balance) -> Self {
        Self {
            account_id,
            balance,
        }
    }

    pub fn from_seed(seed: &str, balance: Balance) -> Self {
        Self::new(get_account_id_from_seed::<sr25519::Public>(seed), balance)
    }

    pub fn json_data(&self) -> (AccountId, Balance) {
        (self.account_id.clone(), self.balance)
    }
}

fn from_ss58check<T: sp_core::crypto::Ss58Codec>(
    key: &str,
) -> Result<T, sp_core::crypto::PublicError> {
    <T as sp_core::crypto::Ss58Codec>::from_ss58check(key)
}

fn ids(addr: &str) -> Ids {
    (
        from_ss58check(addr).expect("Invalid collator account"),
        from_ss58check(addr).expect("Invalid collator account")
    )
}

fn volta_staging_config_genesis() -> serde_json::Value {
    let para_id = PARA_ID.into();
    let balances = vec![];
    let initial_authorities = [
        "5EZ38gMf2TD8dSTDcN1uXeRgd8nrBmmPXsniAUPHsK9r626f",
        "5DJHafz1K8irXXuQW9NVBnsQFFUu9p7PYdHMdqKRAxjqdio1"
    ];
    let sudo_account = "5EM6Tg15hNgoU4WQYRZTgsC43HviRTrFDtaVGY3ERcUye7eB";
    genesis(
        para_id,
        // Initial PoA authorities
        initial_authorities.into_iter()
            .map(ids)
            .collect::<Vec<_>>(),
        // Sudo account
        from_ss58check(sudo_account).expect("Invalid sudo account"),
        // Pre-funded accounts
        balances,
    )
}

pub fn local_config_genesis() -> serde_json::Value {
    let balances = DEFAULT_ENDOWED_SEEDS
        .into_iter()
        .map(|seed| FundedAccount::from_seed(seed, ENDOWMENT))
        .collect::<Vec<_>>();

    let authorities_num = 2;
    let initial_authorities = DEFAULT_ENDOWED_SEEDS
        .iter()
        .take(authorities_num)
        .map(|seed|
            (
                get_account_id_from_seed::<sr25519::Public>(seed),
                get_from_seed::<AuraId>(seed),
            )
        )
        .collect::<Vec<_>>();

    genesis(
        PARA_ID.into(),
        // Initial PoA authorities
        initial_authorities,
        // Sudo account
        get_account_id_from_seed::<sr25519::Public>(DEFAULT_ENDOWED_SEEDS[0]),
        // Pre-funded accounts
        balances
            .iter()
            .map(FundedAccount::json_data)
            .collect::<Vec<_>>(),
    )
}

pub fn development_config_genesis() -> serde_json::Value {
    let balances = DEFAULT_ENDOWED_SEEDS
        .into_iter()
        .map(|seed| FundedAccount::from_seed(seed, ENDOWMENT))
        .chain([
            // The following is a workaround for pallet_treasury benchmarks which hardcode
            // a payment of 100 (lower than EXISTENTIAL_DEPOSIT) to a given address ([0x0])
            #[cfg(feature = "runtime-benchmarks")]
            (FundedAccount::from_id(
                "5C4hrfjw9DjXZTzV3MwzrrAr9P1MJhSrvWGWqi1eSuyUpnhM",
                ENDOWMENT,
            )
            .expect("Address not valid")),
        ])
        .collect::<Vec<_>>();

    let authorities_num = 1;
    let initial_authorities = DEFAULT_ENDOWED_SEEDS
        .iter()
        .take(authorities_num)
        .map(|seed|
            (
                get_account_id_from_seed::<sr25519::Public>(seed),
                get_from_seed::<AuraId>(seed),
            )
        )
        .collect::<Vec<_>>();

    genesis(
        PARA_ID.into(),
        // Initial PoA authorities
        initial_authorities,
        // Sudo account
        get_account_id_from_seed::<sr25519::Public>(DEFAULT_ENDOWED_SEEDS[0]),
        // Pre-funded accounts
        balances
            .iter()
            .map(FundedAccount::json_data)
            .collect::<Vec<_>>(),
    )
}

pub fn preset_names() -> Vec<PresetId> {
    vec![PresetId::from("development"), PresetId::from("local"), PresetId::from("staging")]
}

pub fn get_preset(id: &sp_genesis_builder::PresetId) -> Option<Vec<u8>> {
    let cfg = match id.as_ref() {
        "development" => development_config_genesis(),
        "local" => local_config_genesis(),
        "volta" => volta_staging_config_genesis(),
        _ => return None,
    };
    Some(
        serde_json::to_string(&cfg)
            .expect("genesis cfg must be serializable. qed.")
            .into_bytes(),
    )
}

/// Configure initial storage state for FRAME modules.
#[allow(clippy::too_many_arguments)]
fn genesis(
    id: ParaId,
    initial_collators: Vec<Ids>,
    root_key: AccountId,
    endowed_accounts: Vec<(AccountId, Balance)>,
) -> serde_json::Value {
    #[cfg(feature = "runtime-benchmarks")]
    let endowed_accounts = endowed_accounts
        .into_iter()
        .chain(Some((
            get_from_seed_url::<sp_core::ecdsa::Public>("//Bob").into(),
            ENDOWMENT,
        )))
        .collect::<Vec<_>>();

    serde_json::json!({
        "balances": {
            // Configure endowed accounts with initial balance.
            "balances": endowed_accounts,
        },
        "parachainInfo": {
            "parachainId": id,
        },
        "session": {
            "keys": initial_collators.iter()
                .cloned()
                .map(|(account, aura)| { (account.clone(), account, SessionKeys { aura }) })
                .collect::<Vec<_>>(),
        },
        "collatorSelection": {
            "invulnerables": initial_collators.into_iter().map(|(acc, _)| acc).collect::<Vec<_>>(),
            "candidacyBond": 100,
            "desiredCandidates": 0,
        },
        "xcmPallet": {
            "safeXcmVersion": Some(SAFE_XCM_VERSION),
        },
        "sudo": { "key": Some(root_key) },
    })
}

