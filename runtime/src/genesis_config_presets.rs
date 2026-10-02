use cumulus_primitives_core::ParaId;

use crate::{
	AccountId, BalancesConfig, CollatorSelectionConfig, ParachainInfoConfig, PolkadotXcmConfig,
	RuntimeGenesisConfig, SessionConfig, SessionKeys, TxPauseConfig, EXISTENTIAL_DEPOSIT,
	// Membership - Technical council (sudo replacement) and Treasury council
	TechnicalCommitteeMembershipConfig,configs::TechnicalMembershipMaxMembers,
	TreasuryCouncilMembershipConfig,configs::TreasuryMembershipMaxMembers,
};
use alloc::{vec, vec::Vec};
//use parachains_common::{genesis_config_helpers::*, AuraId};
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use serde_json::Value;
//use sp_genesis_builder::PresetId;
use sp_genesis_builder::{self, PresetId};
use sp_runtime::{BoundedVec, AccountId32};
use sp_keyring::Sr25519Keyring;

/// The default XCM version to set in genesis config.
const SAFE_XCM_VERSION: u32 = xcm::prelude::XCM_VERSION;

#[docify::export_content]
pub const PARACHAIN_ID: u32 = 3417;

/// Generate the session keys from individual elements.
///
/// The input must be a tuple of individual keys (a single arg for now since we have just one key).
pub fn template_session_keys(keys: AuraId) -> SessionKeys {
	SessionKeys { aura: keys }
}

fn testnet_genesis(
	invulnerables: Vec<(AccountId, AuraId)>,
	endowed_accounts: Vec<AccountId>,
	technical_committee_members: Vec<AccountId>,
	treasury_council_members: Vec<AccountId>,
	id: ParaId,
) -> Value {

	// Technical council
	let technical_committee_members: Vec<AccountId32> = technical_committee_members.clone();
    let bounded_technical_committee_members = BoundedVec::<_, TechnicalMembershipMaxMembers>::try_from(
        technical_committee_members.clone(),
    ).expect("Technical council members exceed the allowed limit");

	// Treasury council
	let treasury_council_members: Vec<AccountId32> = treasury_council_members.clone();
    let bounded_treasury_council_members = BoundedVec::<_, TreasuryMembershipMaxMembers>::try_from(
        treasury_council_members.clone(),
    ).expect("Treasury council members exceed the allowed limit");

	let config = RuntimeGenesisConfig {
		balances: BalancesConfig {
			balances: endowed_accounts
				.iter()
				.cloned()
				.map(|k| (k, 1u128 << 60))
				.collect::<Vec<_>>(),
			dev_accounts: None,
		},
		parachain_info: ParachainInfoConfig { parachain_id: id, ..Default::default() },
		collator_selection: CollatorSelectionConfig {
			invulnerables: invulnerables.iter().cloned().map(|(acc, _)| acc).collect::<Vec<_>>(),
			candidacy_bond: EXISTENTIAL_DEPOSIT * 16,
			..Default::default()
		},
		session: SessionConfig {
			keys: invulnerables
				.into_iter()
				.map(|(acc, aura)| {
					(
						acc.clone(),                 // account id
						acc,                         // validator id
						template_session_keys(aura), // session keys
					)
				})
				.collect::<Vec<_>>(),
			..Default::default()
		},
		polkadot_xcm: PolkadotXcmConfig {
			safe_xcm_version: Some(SAFE_XCM_VERSION),
			..Default::default()
		},
		tx_pause: TxPauseConfig {
			paused: vec![
				(
					b"AssetConversion".to_vec().try_into().expect("pallet name fits MaxNameLen"),
					b"swap_exact_tokens_for_tokens"
						.to_vec()
						.try_into()
						.expect("call name fits MaxNameLen"),
				),
				(
					b"AssetConversion".to_vec().try_into().expect("pallet name fits MaxNameLen"),
					b"swap_tokens_for_exact_tokens"
						.to_vec()
						.try_into()
						.expect("call name fits MaxNameLen"),
				),
			],
			..Default::default()
		},
		technical_committee_membership: TechnicalCommitteeMembershipConfig {
            members: bounded_technical_committee_members,
            phantom: Default::default(),
        },
		treasury_council_membership: TreasuryCouncilMembershipConfig {
            members: bounded_treasury_council_members,
            phantom: Default::default(),
        },
		..Default::default()
	};

	serde_json::to_value(config).expect("Could not build genesis config.")
}

fn local_testnet_genesis() -> Value {
	testnet_genesis(
		// initial collators.
		vec![
			(
				Sr25519Keyring::Alice.to_account_id(),
				Sr25519Keyring::Alice.public().into(),
			),
			(
				Sr25519Keyring::Bob.to_account_id(),
				Sr25519Keyring::Bob.public().into(),
			),
		],
		vec![
			Sr25519Keyring::Alice.to_account_id(),
			Sr25519Keyring::Bob.to_account_id(),
			Sr25519Keyring::Charlie.to_account_id(),
			Sr25519Keyring::Dave.to_account_id(),
			Sr25519Keyring::Eve.to_account_id(),
			Sr25519Keyring::Ferdie.to_account_id(),
		],
		vec![
			Sr25519Keyring::Alice.to_account_id(),
			Sr25519Keyring::Bob.to_account_id(),
			Sr25519Keyring::Charlie.to_account_id(),
		],
		vec![
			Sr25519Keyring::Dave.to_account_id(),
			Sr25519Keyring::Eve.to_account_id(),
			Sr25519Keyring::Ferdie.to_account_id(),
		],
		PARACHAIN_ID.into(),
	)
}

fn development_config_genesis() -> Value {
	testnet_genesis(
		// initial collators.
		vec![
			(
				Sr25519Keyring::Alice.to_account_id(),
				Sr25519Keyring::Alice.public().into(),
			),
			(
				Sr25519Keyring::Bob.to_account_id(),
				Sr25519Keyring::Bob.public().into(),
			),
		],
		vec![
			Sr25519Keyring::Alice.to_account_id(),
			Sr25519Keyring::Bob.to_account_id(),
			Sr25519Keyring::Charlie.to_account_id(),
			Sr25519Keyring::Dave.to_account_id(),
			Sr25519Keyring::Eve.to_account_id(),
			Sr25519Keyring::Ferdie.to_account_id(),
		],
		vec![
			Sr25519Keyring::Alice.to_account_id(),
			Sr25519Keyring::Bob.to_account_id(),
			Sr25519Keyring::Charlie.to_account_id(),
		],
		vec![
			Sr25519Keyring::Dave.to_account_id(),
			Sr25519Keyring::Eve.to_account_id(),
			Sr25519Keyring::Ferdie.to_account_id(),
		],
		PARACHAIN_ID.into(),
	)
}

/// Provides the JSON representation of predefined genesis config for given `id`.
pub fn get_preset(id: &PresetId) -> Option<vec::Vec<u8>> {
	let patch = match id.as_ref() {
		sp_genesis_builder::LOCAL_TESTNET_RUNTIME_PRESET => local_testnet_genesis(),
		sp_genesis_builder::DEV_RUNTIME_PRESET => development_config_genesis(),
		_ => return None,
	};
	Some(
		serde_json::to_string(&patch)
			.expect("serialization to json is expected to work. qed.")
			.into_bytes(),
	)
}

/// List of supported presets.
pub fn preset_names() -> Vec<PresetId> {
	vec![
		PresetId::from(sp_genesis_builder::DEV_RUNTIME_PRESET),
		PresetId::from(sp_genesis_builder::LOCAL_TESTNET_RUNTIME_PRESET),
	]
}
