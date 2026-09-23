// This is free and unencumbered software released into the public domain.
//
// Anyone is free to copy, modify, publish, use, compile, sell, or
// distribute this software, either in source code form or as a compiled
// binary, for any purpose, commercial or non-commercial, and by any
// means.
//
// In jurisdictions that recognize copyright laws, the author or authors
// of this software dedicate any and all copyright interest in the
// software to the public domain. We make this dedication for the benefit
// of the public at large and to the detriment of our heirs and
// successors. We intend this dedication to be an overt act of
// relinquishment in perpetuity of all present and future rights to this
// software under copyright law.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
// EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.
// IN NO EVENT SHALL THE AUTHORS BE LIABLE FOR ANY CLAIM, DAMAGES OR
// OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE,
// ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR
// OTHER DEALINGS IN THE SOFTWARE.
//
// For more information, please refer to <http://unlicense.org>
#![cfg_attr(not(feature = "std"), no_std)]
// `construct_runtime!` does a lot of recursion and requires us to increase the limit to 256.
#![recursion_limit = "256"]

// Make the WASM binary available.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

#[cfg(feature = "runtime-benchmarks")]
mod benchmarks;

pub mod configs;

mod genesis_config_presets;
mod weights;

extern crate alloc;
use alloc::{vec, vec::Vec, sync::Arc};
use smallvec::smallvec;

#[cfg(any(feature = "std", test))]
pub use sp_runtime::BuildStorage;

use sp_runtime::{
	generic, impl_opaque_keys,
	traits::{BlakeTwo256, IdentifyAccount, Verify},
	MultiSignature, MultiAddress, Perbill,
	traits::Block as BlockT,
	transaction_validity::{TransactionSource, TransactionValidity},
	ApplyExtrinsicResult,
};

#[cfg(feature = "std")]
use sp_version::NativeVersion;
use sp_version::RuntimeVersion;

use frame_support::{
	genesis_builder_helper::{build_state, get_preset},
	weights::{
		constants::WEIGHT_REF_TIME_PER_SECOND, Weight, WeightToFeeCoefficient, WeightToFeeCoefficients,
		WeightToFeePolynomial, WeightToFee as WeightToFeeConversion,
	},
};

pub use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use pallet_aura::Authorities;

use weights::ExtrinsicBaseWeight;

use sp_api::impl_runtime_apis;
use sp_core::{crypto::KeyTypeId, OpaqueMetadata};

use configs::{
	RuntimeBlockWeights,
	xcm_config::{
		XcmConfig, XcmRouter, LocationToAccountId,
		weight_trader::{WeightToFeeConverter, XonWeightToFeeRate, DotWeightToFeeRate, UsdtWeightToFeeRate}
	}
};

use xcm::{
	latest::prelude::{
		AssetId, Junctions, Junction, Location,
	},
	Version as XcmVersion, VersionedAssetId, VersionedAssets, VersionedLocation,
	VersionedXcm,
};
use xcm_runtime_apis::{
	dry_run::{
		CallDryRunEffects as ApiCallDryRunEffects, 
		XcmDryRunEffects as ApiXcmDryRunEffects,
		Error as XcmDryRunApiError,
	},
	fees::Error as XcmPaymentApiError,
};

/// Revive
use pallet_revive::evm::runtime::EthExtra;

use sp_std::borrow::Cow;

/// Alias to 512-bit hash when used in the context of a transaction signature on the chain.
pub type Signature = MultiSignature;

/// Some way of identifying an account on the chain. We intentionally make it equivalent
/// to the public key of our transaction signing scheme.
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

/// Balance of an account.
pub type Balance = u128;

/// Index of a transaction in the chain.
pub type Nonce = u32;

/// A hash of some data used by the chain.
pub type Hash = sp_core::H256;

/// An index to a block.
pub type BlockNumber = u32;

/// The address format for describing accounts.
pub type Address = MultiAddress<AccountId, ()>;

/// Block header type as expected by this runtime.
pub type Header = generic::Header<BlockNumber, BlakeTwo256>;

/// Block type as expected by this runtime.
pub type Block = generic::Block<Header, UncheckedExtrinsic>;

/// A Block signed with a Justification
pub type SignedBlock = generic::SignedBlock<Block>;

/// BlockId type as expected by this runtime.
pub type BlockId = generic::BlockId<Block>;

/// The SignedExtension to the basic transaction logic.
#[docify::export(template_signed_extra)]
pub type TxExtension = cumulus_pallet_weight_reclaim::StorageWeightReclaim<
    Runtime,
    (
        frame_system::CheckNonZeroSender<Runtime>,
        frame_system::CheckSpecVersion<Runtime>,
        frame_system::CheckTxVersion<Runtime>,
        frame_system::CheckGenesis<Runtime>,
        frame_system::CheckEra<Runtime>,
        frame_system::CheckNonce<Runtime>,
        frame_system::CheckWeight<Runtime>,
        pallet_transaction_payment::ChargeTransactionPayment<Runtime>,
        frame_metadata_hash_extension::CheckMetadataHash<Runtime>,
		pallet_revive::evm::tx_extension::SetOrigin<Runtime>,
    ),
>;

/// Pallet contracts
const CONTRACTS_DEBUG_OUTPUT: pallet_contracts::DebugInfo =
    pallet_contracts::DebugInfo::UnsafeDebug;
const CONTRACTS_EVENTS: pallet_contracts::CollectEvents =
    pallet_contracts::CollectEvents::UnsafeCollect;
	
/// EthExtra converts an unsigned Call::eth_transact into a CheckedExtrinsic.
/// Default extensions applied to Ethereum transactions.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EthExtraImpl;

impl EthExtra for EthExtraImpl {
	type Config = Runtime;
	type Extension = TxExtension;

	fn get_eth_extension(nonce: u32, tip: Balance) -> Self::Extension {
		(
            frame_system::CheckNonZeroSender::<Runtime>::new(),
            frame_system::CheckSpecVersion::<Runtime>::new(),
            frame_system::CheckTxVersion::<Runtime>::new(),
            frame_system::CheckGenesis::<Runtime>::new(),
            frame_system::CheckMortality::from(generic::Era::Immortal),
            frame_system::CheckNonce::<Runtime>::from(nonce),
            frame_system::CheckWeight::<Runtime>::new(),
            pallet_transaction_payment::ChargeTransactionPayment::<Runtime>::from(tip),
            frame_metadata_hash_extension::CheckMetadataHash::<Runtime>::new(false),
			pallet_revive::evm::tx_extension::SetOrigin::<Runtime>::new_from_eth_transaction(),
		)
			.into()
	}
}

/// Unchecked extrinsic type as expected by this runtime.
pub type UncheckedExtrinsic =
	pallet_revive::evm::runtime::UncheckedExtrinsic<Address, Signature, EthExtraImpl>;

/// Executive: handles dispatch to the various modules.
pub type Executive = frame_executive::Executive<
	Runtime,
	Block,
	frame_system::ChainContext<Runtime>,
	Runtime,
	AllPalletsWithSystem,
>;

/// Handles converting a weight scalar to a fee value, based on the scale and granularity of the
/// node's balance type.
///
/// This should typically create a mapping between the following ranges:
///   - `[0, MAXIMUM_BLOCK_WEIGHT]`
///   - `[Balance::min, Balance::max]`
///
/// Yet, it can be used for any other sort of change to weight-fee. Some examples being:
///   - Setting it to `0` will essentially disable the weight fee.
///   - Setting it to `1` will cause the literal `#[weight = x]` values to be charged.
pub struct WeightToFee;
impl WeightToFeePolynomial for WeightToFee {
	type Balance = Balance;
	fn polynomial() -> WeightToFeeCoefficients<Self::Balance> {
		// in Rococo, extrinsic base weight (smallest non-zero weight) is mapped to 1 MILLI_UNIT:
		// in our template, we map to 1/10 of that, or 1/10 MILLI_UNIT
		let p = MILLI_UNIT / 10;
		let q = 100 * Balance::from(ExtrinsicBaseWeight::get().ref_time());
		smallvec![WeightToFeeCoefficient {
			degree: 1,
			negative: false,
			coeff_frac: Perbill::from_rational(p % q, q),
			coeff_integer: p / q,
		}]
	}
}

/// Opaque types. These are used by the CLI to instantiate machinery that don't need to know
/// the specifics of the runtime. They can then be made to be agnostic over specific formats
/// of data like extrinsics, allowing for them to continue syncing the network through upgrades
/// to even the core data structures.
pub mod opaque {
	use super::*;
	use sp_runtime::{
		generic,
		traits::{BlakeTwo256, Hash as HashT},
	};

	pub use sp_runtime::OpaqueExtrinsic as UncheckedExtrinsic;

	/// Opaque block header type.
	pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
	/// Opaque block type.
	pub type Block = generic::Block<Header, UncheckedExtrinsic>;
	/// Opaque block identifier type.
	pub type BlockId = generic::BlockId<Block>;
	/// Opaque block hash type.
	pub type Hash = <BlakeTwo256 as HashT>::Output;
}

impl_opaque_keys! {
	pub struct SessionKeys {
		pub aura: Aura,
	}
}

#[sp_version::runtime_version]
pub const VERSION: RuntimeVersion = RuntimeVersion {
	spec_name: Cow::Borrowed("xode-runtime"),
	impl_name: Cow::Borrowed("xode-runtime"),
	authoring_version: 1,
	spec_version: 14,
	impl_version: 0,
	apis: RUNTIME_API_VERSIONS,
	transaction_version: 1,
	system_version: 1,
};

#[docify::export]
mod block_times {
	/// This determines the average expected block time that we are targeting. Blocks will be
	/// produced at a minimum duration defined by `SLOT_DURATION`. `SLOT_DURATION` is picked up by
	/// `pallet_timestamp` which is in turn picked up by `pallet_aura` to implement `fn
	/// slot_duration()`.
	///
	/// Change this to adjust the block time.
	pub const MILLI_SECS_PER_BLOCK: u64 = 6000;

	// NOTE: Currently it is not possible to change the slot duration after the chain has started.
	// Attempting to do so will brick block production.
	pub const SLOT_DURATION: u64 = MILLI_SECS_PER_BLOCK;
}
pub use block_times::*;

// Time is measured by number of blocks.
pub const MINUTES: BlockNumber = 60_000 / (MILLI_SECS_PER_BLOCK as BlockNumber);
pub const HOURS: BlockNumber = MINUTES * 60;
pub const DAYS: BlockNumber = HOURS * 24;

// Unit = the base number of indivisible units for balances
pub const UNIT: Balance = 1_000_000_000_000;
pub const MILLI_UNIT: Balance = 1_000_000_000;
pub const MICRO_UNIT: Balance = 1_000_000;

/// The existential deposit. Set to 1/10 of the Connected Relay Chain.
pub const EXISTENTIAL_DEPOSIT: Balance = MILLI_UNIT;

/// We assume that ~5% of the block weight is consumed by `on_initialize` handlers. This is
/// used to limit the maximal weight of a single extrinsic.
const AVERAGE_ON_INITIALIZE_RATIO: Perbill = Perbill::from_percent(5);

/// We allow `Normal` extrinsics to fill up the block up to 75%, the rest can be used by
/// `Operational` extrinsics.
const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);

#[docify::export(max_block_weight)]
/// We allow for 2 seconds of compute with a 6 second average block time.
const MAXIMUM_BLOCK_WEIGHT: Weight = Weight::from_parts(
	WEIGHT_REF_TIME_PER_SECOND.saturating_mul(2),
	cumulus_primitives_core::relay_chain::MAX_POV_SIZE as u64,
);

#[docify::export]
mod async_backing_params {
	/// Maximum number of blocks simultaneously accepted by the Runtime, not yet included
	/// into the relay chain.
	pub(crate) const UNINCLUDED_SEGMENT_CAPACITY: u32 = 3;
	/// How many parachain blocks are processed by the relay chain per parent. Limits the
	/// number of blocks authored per slot.
	pub(crate) const BLOCK_PROCESSING_VELOCITY: u32 = 1;
	/// Relay chain slot duration, in milliseconds.
	pub(crate) const RELAY_CHAIN_SLOT_DURATION_MILLIS: u32 = 6000;
}
pub(crate) use async_backing_params::*;

#[docify::export]
/// Aura consensus hook
type ConsensusHook = cumulus_pallet_aura_ext::FixedVelocityConsensusHook<
	Runtime,
	RELAY_CHAIN_SLOT_DURATION_MILLIS,
	BLOCK_PROCESSING_VELOCITY,
	UNINCLUDED_SEGMENT_CAPACITY,
>;

/// The version information used to identify this runtime when compiled natively.
#[cfg(feature = "std")]
pub fn native_version() -> NativeVersion {
	NativeVersion { runtime_version: VERSION, can_author_with: Default::default() }
}

// Create the runtime by composing the FRAME pallets that were previously configured.
#[frame_support::runtime]
mod runtime {
	#[runtime::runtime]
	#[runtime::derive(
		RuntimeCall,
		RuntimeEvent,
		RuntimeError,
		RuntimeOrigin,
		RuntimeFreezeReason,
		RuntimeHoldReason,
		RuntimeSlashReason,
		RuntimeLockId,
		RuntimeTask
	)]
	pub struct Runtime;

	#[runtime::pallet_index(0)]
	pub type System = frame_system;
	#[runtime::pallet_index(1)]
	pub type ParachainSystem = cumulus_pallet_parachain_system;
	#[runtime::pallet_index(2)]
	pub type Timestamp = pallet_timestamp;
	#[runtime::pallet_index(3)]
	pub type ParachainInfo = parachain_info;

	// Monetary stuff.
	#[runtime::pallet_index(10)]
	pub type Balances = pallet_balances;
	#[runtime::pallet_index(11)]
	pub type TransactionPayment = pallet_transaction_payment;

	// Governance
	// #[runtime::pallet_index(15)]
	// pub type Sudo = pallet_sudo;

	// Collator support. The order of these 4 are important and shall not change.
	#[runtime::pallet_index(20)]
	pub type Authorship = pallet_authorship;
	#[runtime::pallet_index(21)]
	pub type CollatorSelection = pallet_collator_selection;
	#[runtime::pallet_index(22)]
	pub type Session = pallet_session;
	#[runtime::pallet_index(23)]
	pub type Aura = pallet_aura;
	#[runtime::pallet_index(24)]
	pub type AuraExt = cumulus_pallet_aura_ext;

	// XCM helpers.
	#[runtime::pallet_index(30)]
	pub type XcmpQueue = cumulus_pallet_xcmp_queue;
	#[runtime::pallet_index(31)]
	pub type PolkadotXcm = pallet_xcm;
	#[runtime::pallet_index(32)]
	pub type CumulusXcm = cumulus_pallet_xcm;
	#[runtime::pallet_index(33)]
	pub type MessageQueue = pallet_message_queue;

	// Frames (Xode Blockchain)
	#[runtime::pallet_index(50)]
	pub type Assets = pallet_assets::Pallet<Runtime, Instance1>;
	#[runtime::pallet_index(51)]
	pub type Contracts = pallet_contracts;
	#[runtime::pallet_index(52)]
	pub type Indices = pallet_indices;
	#[runtime::pallet_index(53)]
	pub type AssetRate = pallet_asset_rate;
	#[runtime::pallet_index(54)]
	pub type Treasury = pallet_treasury;
	#[runtime::pallet_index(55)]
	pub type TechnicalCommittee = pallet_collective::Pallet<Runtime, Instance1>;
	#[runtime::pallet_index(56)]
	pub type TechnicalCommitteeMembership = pallet_membership::Pallet<Runtime, Instance1>;
	#[runtime::pallet_index(57)]
	pub type TreasuryCouncil = pallet_collective::Pallet<Runtime, Instance2>;
	#[runtime::pallet_index(58)]
	pub type TreasuryCouncilMembership = pallet_membership::Pallet<Runtime, Instance2>;
	#[runtime::pallet_index(59)]
    pub type Preimage = pallet_preimage;
	#[runtime::pallet_index(60)]
    pub type Whitelist = pallet_whitelist;

	// Pallet (Xode Blockchain)
	#[runtime::pallet_index(70)]
	pub type XodeStaking = pallet_xode_staking;

	#[runtime::pallet_index(71)]
	pub type AccountFreezer = pallet_xode_account_freezer;

	// Utility
	#[runtime::pallet_index(80)]	
	pub type Utility = pallet_utility;
	#[runtime::pallet_index(81)]	
	pub type RootTesting = pallet_root_testing;

	// Revive
    #[runtime::pallet_index(90)]
    pub type Revive = pallet_revive;

	// Asset Conversion
	#[runtime::pallet_index(100)]
  pub type AssetConversion = pallet_asset_conversion;
	#[runtime::pallet_index(101)]
  pub type ForeignAssets = pallet_assets::Pallet<Runtime, Instance2>;
	#[runtime::pallet_index(102)]
  pub type PoolAssets = pallet_assets::Pallet<Runtime, Instance3>;
	#[runtime::pallet_index(103)]
  pub type AssetsFreezer = pallet_assets_freezer::Pallet<Runtime, Instance1>;
	#[runtime::pallet_index(104)]
  pub type ForeignAssetsFreezer = pallet_assets_freezer::Pallet<Runtime, Instance2>;
	#[runtime::pallet_index(105)]
  pub type PoolAssetsFreezer = pallet_assets_freezer::Pallet<Runtime, Instance3>;
	#[runtime::pallet_index(106)] 
  pub type AssetConversionOps = pallet_asset_conversion_ops;
}

#[docify::export(register_validate_block)]
cumulus_pallet_parachain_system::register_validate_block! {
	Runtime = Runtime,
	BlockExecutor = cumulus_pallet_aura_ext::BlockExecutor::<Runtime, Executive>,
}

// These runtime API implementations were originally placed in api.rs.
//
// However, PAPI was unable to detect them when they were defined
// in a separate file. This happens because construct_runtime! relies on
// autoref-based specialization to fetch metadata about runtime APIs,
// and the detection fails if the APIs are split out.
// 
// To fix this, the impl_runtime_apis! trait was moved into the metadata-ir
// crate, which ensures proper detection and allows PAPI to work correctly.
// 
// See: https://github.com/paritytech/polkadot-sdk/pull/6665
type EventRecord = frame_system::EventRecord<
    <Runtime as frame_system::Config>::RuntimeEvent,
    <Runtime as frame_system::Config>::Hash,
>;

// Stable 2512 Update
type LazyBlockOf<T> =
    sp_runtime::generic::LazyBlock<
        <T as sp_runtime::traits::Block>::Header,
        <T as sp_runtime::traits::Block>::Extrinsic,
    >;

// we move some impls outside so we can easily use them with `docify`.
impl Runtime {
	#[docify::export]
	fn impl_slot_duration() -> sp_consensus_aura::SlotDuration {
		sp_consensus_aura::SlotDuration::from_millis(SLOT_DURATION)
	}

	#[docify::export]
	fn impl_can_build_upon(
		included_hash: <Block as BlockT>::Hash,
		slot: cumulus_primitives_aura::Slot,
	) -> bool {
		ConsensusHook::can_build_upon(included_hash, slot)
	}
}

pallet_revive::impl_runtime_apis_plus_revive_traits! (
	Runtime,
	Revive,
	Executive,
	EthExtraImpl,

	impl sp_consensus_aura::AuraApi<Block, AuraId> for Runtime {
		fn slot_duration() -> sp_consensus_aura::SlotDuration {
			Runtime::impl_slot_duration()
		}

		fn authorities() -> Vec<AuraId> {
			Authorities::<Runtime>::get().into_inner()
		}
	}

	impl cumulus_primitives_aura::AuraUnincludedSegmentApi<Block> for Runtime {
		fn can_build_upon(
			included_hash: <Block as BlockT>::Hash,
			slot: cumulus_primitives_aura::Slot,
		) -> bool {
			Runtime::impl_can_build_upon(included_hash, slot)
		}
	}

	impl sp_api::Core<Block> for Runtime {
		fn version() -> RuntimeVersion {
			VERSION
		}

		fn execute_block(block: LazyBlockOf<Block>) {
			Executive::execute_block(block)
		}

		fn initialize_block(header: &<Block as BlockT>::Header) -> sp_runtime::ExtrinsicInclusionMode {
			Executive::initialize_block(header)
		}
	}

	impl sp_api::Metadata<Block> for Runtime {
		fn metadata() -> OpaqueMetadata {
			OpaqueMetadata::new(Runtime::metadata().into())
		}

		fn metadata_at_version(version: u32) -> Option<OpaqueMetadata> {
			Runtime::metadata_at_version(version)
		}

		fn metadata_versions() -> Vec<u32> {
			Runtime::metadata_versions()
		}
	}

	impl sp_block_builder::BlockBuilder<Block> for Runtime {
		fn apply_extrinsic(extrinsic: <Block as BlockT>::Extrinsic) -> ApplyExtrinsicResult {
			Executive::apply_extrinsic(extrinsic)
		}

		fn finalize_block() -> <Block as BlockT>::Header {
			Executive::finalize_block()
		}

		fn inherent_extrinsics(data: sp_inherents::InherentData) -> Vec<<Block as BlockT>::Extrinsic> {
			data.create_extrinsics()
		}

		fn check_inherents(
			block: LazyBlockOf<Block>,
			data: sp_inherents::InherentData,
		) -> sp_inherents::CheckInherentsResult {
			data.check_extrinsics(&block)
		}
	}

	impl sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block> for Runtime {
		fn validate_transaction(
			source: TransactionSource,
			tx: <Block as BlockT>::Extrinsic,
			block_hash: <Block as BlockT>::Hash,
		) -> TransactionValidity {
			Executive::validate_transaction(source, tx, block_hash)
		}
	}

	impl sp_offchain::OffchainWorkerApi<Block> for Runtime {
		fn offchain_worker(header: &<Block as BlockT>::Header) {
			Executive::offchain_worker(header)
		}
	}

	impl sp_session::SessionKeys<Block> for Runtime {
		fn generate_session_keys(seed: Option<Vec<u8>>) -> Vec<u8> {
			SessionKeys::generate(seed)
		}

		fn decode_session_keys(
			encoded: Vec<u8>,
		) -> Option<Vec<(Vec<u8>, KeyTypeId)>> {
			SessionKeys::decode_into_raw_public_keys(&encoded)
		}
	}

	impl frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Nonce> for Runtime {
		fn account_nonce(account: AccountId) -> Nonce {
			System::account_nonce(account)
		}
	}

	impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<Block, Balance> for Runtime {
		fn query_info(
			uxt: <Block as BlockT>::Extrinsic,
			len: u32,
		) -> pallet_transaction_payment_rpc_runtime_api::RuntimeDispatchInfo<Balance> {
			TransactionPayment::query_info(uxt, len)
		}
		fn query_fee_details(
			uxt: <Block as BlockT>::Extrinsic,
			len: u32,
		) -> pallet_transaction_payment::FeeDetails<Balance> {
			TransactionPayment::query_fee_details(uxt, len)
		}
		fn query_weight_to_fee(weight: Weight) -> Balance {
			TransactionPayment::weight_to_fee(weight)
		}
		fn query_length_to_fee(length: u32) -> Balance {
			TransactionPayment::length_to_fee(length)
		}
	}

	impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentCallApi<Block, Balance, RuntimeCall>
		for Runtime
	{
		fn query_call_info(
			call: RuntimeCall,
			len: u32,
		) -> pallet_transaction_payment::RuntimeDispatchInfo<Balance> {
			TransactionPayment::query_call_info(call, len)
		}
		fn query_call_fee_details(
			call: RuntimeCall,
			len: u32,
		) -> pallet_transaction_payment::FeeDetails<Balance> {
			TransactionPayment::query_call_fee_details(call, len)
		}
		fn query_weight_to_fee(weight: Weight) -> Balance {
			TransactionPayment::weight_to_fee(weight)
		}
		fn query_length_to_fee(length: u32) -> Balance {
			TransactionPayment::length_to_fee(length)
		}
	}

	impl pallet_asset_conversion::AssetConversionApi<
		Block,
		Balance,
		xcm::v5::Location,
	> for Runtime
	{
		fn quote_price_exact_tokens_for_tokens(asset1: xcm::v5::Location, asset2: xcm::v5::Location, amount: Balance, include_fee: bool) -> Option<Balance> {
			AssetConversion::quote_price_exact_tokens_for_tokens(asset1, asset2, amount, include_fee)
		}

		fn quote_price_tokens_for_exact_tokens(asset1: xcm::v5::Location, asset2: xcm::v5::Location, amount: Balance, include_fee: bool) -> Option<Balance> {
			AssetConversion::quote_price_tokens_for_exact_tokens(asset1, asset2, amount, include_fee)
		}

		fn get_reserves(asset1: xcm::v5::Location, asset2: xcm::v5::Location) -> Option<(Balance, Balance)> {
			AssetConversion::get_reserves(asset1, asset2).ok()
		}
	}

	impl cumulus_primitives_core::CollectCollationInfo<Block> for Runtime {
		fn collect_collation_info(header: &<Block as BlockT>::Header) -> cumulus_primitives_core::CollationInfo {
			ParachainSystem::collect_collation_info(header)
		}
	}

	impl pallet_contracts::ContractsApi<Block, AccountId, Balance, BlockNumber, Hash, EventRecord>
        for Runtime
    {
        fn call(
            origin: AccountId,
            dest: AccountId,
            value: Balance,
            gas_limit: Option<Weight>,
            storage_deposit_limit: Option<Balance>,
            input_data: Vec<u8>,
        ) -> pallet_contracts::ContractExecResult<Balance, EventRecord> {
            let gas_limit = gas_limit.unwrap_or(RuntimeBlockWeights::get().max_block);
            Contracts::bare_call(
                origin,
                dest,
                value,
                gas_limit,
                storage_deposit_limit,
                input_data,
                CONTRACTS_DEBUG_OUTPUT,
                CONTRACTS_EVENTS,
                pallet_contracts::Determinism::Enforced,
            )
        }
        fn instantiate(
            origin: AccountId,
            value: Balance,
            gas_limit: Option<Weight>,
            storage_deposit_limit: Option<Balance>,
            code: pallet_contracts::Code<Hash>,
            data: Vec<u8>,
            salt: Vec<u8>,
        ) -> pallet_contracts::ContractInstantiateResult<AccountId, Balance, EventRecord>
        {
            let gas_limit = gas_limit.unwrap_or(RuntimeBlockWeights::get().max_block);
            Contracts::bare_instantiate(
                origin,
                value,
                gas_limit,
                storage_deposit_limit,
                code,
                data,
                salt,
                CONTRACTS_DEBUG_OUTPUT,
                CONTRACTS_EVENTS,
            )
        }
        fn upload_code(
            origin: AccountId,
            code: Vec<u8>,
            storage_deposit_limit: Option<Balance>,
            determinism: pallet_contracts::Determinism,
        ) -> pallet_contracts::CodeUploadResult<Hash, Balance>
        {
            Contracts::bare_upload_code(origin, code, storage_deposit_limit, determinism)
        }
        fn get_storage(
            address: AccountId,
            key: Vec<u8>,
        ) -> pallet_contracts::GetStorageResult {
            Contracts::get_storage(address, key)
        }
    }

	#[cfg(feature = "try-runtime")]
	impl frame_try_runtime::TryRuntime<Block> for Runtime {
		fn on_runtime_upgrade(checks: frame_try_runtime::UpgradeCheckSelect) -> (Weight, Weight) {
			use super::configs::RuntimeBlockWeights;

			let weight = Executive::try_runtime_upgrade(checks).unwrap();
			(weight, RuntimeBlockWeights::get().max_block)
		}

		fn execute_block(
			block: Block,
			state_root_check: bool,
			signature_check: bool,
			select: frame_try_runtime::TryStateSelect,
		) -> Weight {
			// NOTE: intentional unwrap: we don't want to propagate the error backwards, and want to
			// have a backtrace here.
			Executive::try_execute_block(block, state_root_check, signature_check, select).unwrap()
		}
	}

	#[cfg(feature = "runtime-benchmarks")]
	impl frame_benchmarking::Benchmark<Block> for Runtime {
		fn benchmark_metadata(extra: bool) -> (
			Vec<frame_benchmarking::BenchmarkList>,
			Vec<frame_support::traits::StorageInfo>,
		) {
			use frame_benchmarking::{Benchmarking, BenchmarkList};
			use frame_support::traits::StorageInfoTrait;
			use frame_system_benchmarking::Pallet as SystemBench;
			use cumulus_pallet_session_benchmarking::Pallet as SessionBench;
			//use super::*;

			let mut list = Vec::<BenchmarkList>::new();
			list_benchmarks!(list, extra);

			let storage_info = AllPalletsWithSystem::storage_info();
			(list, storage_info)
		}

		fn dispatch_benchmark(
			config: frame_benchmarking::BenchmarkConfig
		) -> Result<Vec<frame_benchmarking::BenchmarkBatch>, sp_runtime::RuntimeString> {
			use frame_benchmarking::{BenchmarkError, Benchmarking, BenchmarkBatch};
			//use super::*;

			use frame_system_benchmarking::Pallet as SystemBench;
			impl frame_system_benchmarking::Config for Runtime {
				fn setup_set_code_requirements(code: &Vec<u8>) -> Result<(), BenchmarkError> {
					ParachainSystem::initialize_for_set_code_benchmark(code.len() as u32);
					Ok(())
				}

				fn verify_set_code() {
					System::assert_last_event(cumulus_pallet_parachain_system::Event::<Runtime>::ValidationFunctionStored.into());
				}
			}

			use cumulus_pallet_session_benchmarking::Pallet as SessionBench;
			impl cumulus_pallet_session_benchmarking::Config for Runtime {}

			use frame_support::traits::WhitelistedStorageKeys;
			let whitelist = AllPalletsWithSystem::whitelisted_storage_keys();

			let mut batches = Vec::<BenchmarkBatch>::new();
			let params = (&config, &whitelist);
			add_benchmarks!(params, batches);

			if batches.is_empty() { return Err("Benchmark not found for this pallet.".into()) }
			Ok(batches)
		}
	}

	impl sp_genesis_builder::GenesisBuilder<Block> for Runtime {
		fn build_state(config: Vec<u8>) -> sp_genesis_builder::Result {
			build_state::<RuntimeGenesisConfig>(config)
		}

		fn get_preset(id: &Option<sp_genesis_builder::PresetId>) -> Option<Vec<u8>> {
			get_preset::<RuntimeGenesisConfig>(id, crate::genesis_config_presets::get_preset)
		}

		fn preset_names() -> Vec<sp_genesis_builder::PresetId> {
			crate::genesis_config_presets::preset_names()
		}
	}

	impl xcm_runtime_apis::fees::XcmPaymentApi<Block> for Runtime {
		fn query_acceptable_payment_assets(xcm_version: xcm::Version) -> Result<Vec<VersionedAssetId>, XcmPaymentApiError> {
			let mut acceptable_assets: Vec<AssetId> = Vec::new();

			// XON: Xode native token
			acceptable_assets.push(AssetId(Location {
				parents: 0,
				interior: Junctions::Here,
			}));

			// DOT: Relay chain native token
			acceptable_assets.push(AssetId(Location {
				parents: 1,
				interior: Junctions::Here,
			}));

			// USDT: AssetHub parachain asset (1984)
			acceptable_assets.push(AssetId(Location {
				parents: 1,
				interior: Junctions::X3(Arc::from([
					Junction::Parachain(1000),
					Junction::PalletInstance(50),
					Junction::GeneralIndex(1984u128),
				])),
			}));

			pallet_xcm::Pallet::<Runtime>::query_acceptable_payment_assets(xcm_version, acceptable_assets)
            	.map_err(|_| XcmPaymentApiError::AssetNotFound)
		}

		fn query_xcm_weight(message: VersionedXcm<()>) -> Result<Weight, XcmPaymentApiError> {
			pallet_xcm::Pallet::<Runtime>::query_xcm_weight(message)
				.map_err(|_| XcmPaymentApiError::WeightNotComputable)
		}

		fn query_weight_to_asset_fee(weight: Weight, asset: VersionedAssetId) -> Result<u128, XcmPaymentApiError> {
			// Convert VersionedAssetId to AssetId
			let asset: AssetId = asset.clone()
                .try_into()
                .map_err(|_| XcmPaymentApiError::VersionedConversionFailed)?;

            // Add the fixed fee (0.01 DOT or USDT, depending on asset)
            let total_fee: u128 = match asset {
				// XON: local chain (12 decimals -> 0.01 XON = 10_000_000_000)
                AssetId(Location {
                    parents: 0,
                    interior: Junctions::Here,
                }) => WeightToFeeConverter::<XonWeightToFeeRate>::weight_to_fee(&weight).saturating_add(10_000_000_000u128),

				// DOT: Relay chain (10 decimals -> 0.01 DOT = 100_000_000)
                AssetId(Location {
                    parents: 1,
                    interior: Junctions::Here,
                }) => WeightToFeeConverter::<DotWeightToFeeRate>::weight_to_fee(&weight).saturating_add(100_000_000u128),

				// USDT: AssetHub parachain asset (1984) (6 decimals -> 0.01 USDT = 10_000)
                AssetId(Location {
                    parents: 1,
                    interior: Junctions::X3(ref junctions),
                }) if matches!(
                    junctions.as_ref(),
                    [
                        Junction::Parachain(1000),
                        Junction::PalletInstance(50),
                        Junction::GeneralIndex(1984)
                    ]
                ) => WeightToFeeConverter::<UsdtWeightToFeeRate>::weight_to_fee(&weight).saturating_add(10_000u128),

                _ => return Err(XcmPaymentApiError::AssetNotFound),
            };

            // Ensure the fee does not exceed the maximum payment
            Ok(total_fee)
		}

		fn query_delivery_fees(destination: VersionedLocation, message: VersionedXcm<()>, asset_id: VersionedAssetId) -> Result<VersionedAssets, XcmPaymentApiError> {
			type AssetExchanger = <XcmConfig as xcm_executor::Config>::AssetExchanger;
			pallet_xcm::Pallet::<Runtime>::query_delivery_fees::<AssetExchanger>(destination, message, asset_id)			
		}
	}

	impl xcm_runtime_apis::dry_run::DryRunApi<Block, RuntimeCall, <Runtime as frame_system::Config>::RuntimeEvent, crate::OriginCaller> for Runtime {
		fn dry_run_call(origin: crate::OriginCaller, call: RuntimeCall, result_xcms_version: XcmVersion) -> Result<ApiCallDryRunEffects<<Runtime as frame_system::Config>::RuntimeEvent>, XcmDryRunApiError> {
			pallet_xcm::Pallet::<Runtime>::dry_run_call::<Runtime, XcmRouter, crate::OriginCaller, RuntimeCall>(origin, call, result_xcms_version)
				.map_err(|_| XcmDryRunApiError::Unimplemented)
				.map(|effects| effects.into()) 
		}
		
		fn dry_run_xcm(origin_location: VersionedLocation, xcm: VersionedXcm<RuntimeCall>) -> Result<ApiXcmDryRunEffects<<Runtime as frame_system::Config>::RuntimeEvent>, XcmDryRunApiError> {
			pallet_xcm::Pallet::<Runtime>::dry_run_xcm::<XcmRouter>(origin_location, xcm)
				.map_err(|_| XcmDryRunApiError::Unimplemented)
				.map(|effects| effects.into()) 
		}
	}

	impl xcm_runtime_apis::conversions::LocationToAccountApi<Block, AccountId> for Runtime {
		fn convert_location(location: VersionedLocation) -> Result<
			AccountId,
			xcm_runtime_apis::conversions::Error
		> {
			xcm_runtime_apis::conversions::LocationToAccountHelper::<
				AccountId,
				LocationToAccountId,
			>::convert_location(location)
		}
	}
);
