use crate as pallet_asset_waiting;
use frame_support::{
	construct_runtime, derive_impl,
	instances::{Instance1, Instance2},
	ord_parameter_types, parameter_types,
	traits::{
		tokens::{
			fungible::{NativeFromLeft, NativeOrWithId, UnionOf},
			imbalance::ResolveAssetTo,
		},
		AsEnsureOriginWithArg, ConstU128, ConstU32, Contains, SortedMembers,
	},
	PalletId,
};
use frame_system::{EnsureSigned, EnsureSignedBy};
use pallet_asset_conversion::{
	self,
	AccountIdConverter, Ascending, Chain, Config as AssetConversionConfig, WithFirstAsset,
};
use sp_runtime::{
	Permill,
	traits::{AccountIdConversion, IdentityLookup},
	BuildStorage,
};

pub type AccountId = u128;
pub type Balance = u128;
pub type AssetKind = NativeOrWithId<u32>;
pub type Block = frame_system::mocking::MockBlock<Test>;

construct_runtime!(
	pub enum Test {
		System: frame_system,
		Balances: pallet_balances,
		Assets: pallet_assets::<Instance1>,
		PoolAssets: pallet_assets::<Instance2>,
		AssetConversion: pallet_asset_conversion,
		AssetWaiting: pallet_asset_waiting,
	}
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Block = Block;
	type AccountData = pallet_balances::AccountData<Balance>;
	type BaseCallFilter = AssetWaiting;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
	type Balance = Balance;
	type ExistentialDeposit = ConstU128<100>;
	type AccountStore = System;
}

impl pallet_assets::Config<Instance1> for Test {
	type RuntimeEvent = RuntimeEvent;
	type Balance = Balance;
	type RemoveItemsLimit = ConstU32<1000>;
	type AssetId = u32;
	type AssetIdParameter = u32;
	type ReserveData = ();
	type Currency = Balances;
	type CreateOrigin = AsEnsureOriginWithArg<EnsureSigned<Self::AccountId>>;
	type ForceOrigin = frame_system::EnsureRoot<Self::AccountId>;
	type AssetDeposit = ConstU128<1>;
	type AssetAccountDeposit = ConstU128<10>;
	type MetadataDepositBase = ConstU128<1>;
	type MetadataDepositPerByte = ConstU128<1>;
	type ApprovalDeposit = ConstU128<1>;
	type StringLimit = ConstU32<50>;
	type Holder = ();
	type Freezer = ();
	type Extra = ();
	type WeightInfo = ();
	type CallbackHandle = ();
	pallet_assets::runtime_benchmarks_enabled! {
		type BenchmarkHelper = ();
	}
}

impl pallet_assets::Config<Instance2> for Test {
	type RuntimeEvent = RuntimeEvent;
	type Balance = Balance;
	type RemoveItemsLimit = ConstU32<1000>;
	type AssetId = u32;
	type AssetIdParameter = u32;
	type ReserveData = ();
	type Currency = Balances;
	type CreateOrigin =
		AsEnsureOriginWithArg<EnsureSignedBy<AssetConversionOrigin, Self::AccountId>>;
	type ForceOrigin = frame_system::EnsureRoot<Self::AccountId>;
	type AssetDeposit = ConstU128<0>;
	type AssetAccountDeposit = ConstU128<0>;
	type MetadataDepositBase = ConstU128<0>;
	type MetadataDepositPerByte = ConstU128<0>;
	type ApprovalDeposit = ConstU128<0>;
	type StringLimit = ConstU32<50>;
	type Holder = ();
	type Freezer = ();
	type Extra = ();
	type WeightInfo = ();
	type CallbackHandle = ();
	pallet_assets::runtime_benchmarks_enabled! {
		type BenchmarkHelper = ();
	}
}

parameter_types! {
	pub const AssetConversionPalletId: PalletId = PalletId(*b"py/ascon");
	pub const Native: NativeOrWithId<u32> = NativeOrWithId::Native;
	pub storage LiquidityWithdrawalFee: Permill = Permill::from_percent(0);
}

pub struct GovernanceMembers;

impl SortedMembers<AccountId> for GovernanceMembers {
	fn sorted_members() -> Vec<AccountId> {
		vec![10, 11, 12]
	}
}

ord_parameter_types! {
	pub const AssetConversionOrigin: u128 = AccountIdConversion::<u128>::into_account_truncating(&AssetConversionPalletId::get());
}

pub type NativeAndAssets = UnionOf<Balances, Assets, NativeFromLeft, NativeOrWithId<u32>, u128>;
pub type PoolIdToAccountId =
	AccountIdConverter<AssetConversionPalletId, (NativeOrWithId<u32>, NativeOrWithId<u32>)>;
pub type AscendingLocator = Ascending<u128, NativeOrWithId<u32>, PoolIdToAccountId>;
pub type WithFirstAssetLocator =
	WithFirstAsset<Native, u128, NativeOrWithId<u32>, PoolIdToAccountId>;

impl AssetConversionConfig for Test {
	type RuntimeEvent = RuntimeEvent;
	type Balance = Balance;
	type HigherPrecisionBalance = sp_core::U256;
	type AssetKind = AssetKind;
	type Assets = NativeAndAssets;
	type PoolId = (Self::AssetKind, Self::AssetKind);
	type PoolLocator = Chain<WithFirstAssetLocator, AscendingLocator>;
	type PoolAssetId = u32;
	type PoolAssets = PoolAssets;
	type PoolSetupFee = ConstU128<100>;
	type PoolSetupFeeAsset = Native;
	type PoolSetupFeeTarget = ResolveAssetTo<AssetConversionOrigin, Self::Assets>;
	type PalletId = AssetConversionPalletId;
	type WeightInfo = ();
	type LPFee = ConstU32<3>;
	type LiquidityWithdrawalFee = LiquidityWithdrawalFee;
	type MaxSwapPathLength = ConstU32<4>;
	type MintMinLiquidity = ConstU128<100>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = ();
}

impl crate::pallet::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type GovernanceMembers = GovernanceMembers;
	type WeightInfo = crate::weights::SubstrateWeight<Test>;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut storage = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

	pallet_balances::GenesisConfig::<Test> {
		balances: vec![(1, 100_000), (2, 100_000)],
		..Default::default()
	}
	.assimilate_storage(&mut storage)
	.unwrap();

	let mut ext = sp_io::TestExternalities::new(storage);
	ext.execute_with(|| System::set_block_number(1));
	ext
}

pub fn create_tokens(owner: u128, tokens: Vec<NativeOrWithId<u32>>) {
	for token_id in tokens {
		let asset_id = match token_id {
			NativeOrWithId::WithId(id) => id,
			_ => unreachable!("invalid token"),
		};
		assert!(Assets::force_create(RuntimeOrigin::root(), asset_id, owner, false, 1).is_ok());
	}
}

pub fn swap_call(path: Vec<Box<NativeOrWithId<u32>>>, who: u128) -> RuntimeCall {
	RuntimeCall::AssetConversion(pallet_asset_conversion::Call::swap_exact_tokens_for_tokens {
		path,
		amount_in: 10,
		amount_out_min: 1,
		send_to: who,
		keep_alive: false,
	})
}

pub fn create_pool_call(asset1: NativeOrWithId<u32>, asset2: NativeOrWithId<u32>) -> RuntimeCall {
	RuntimeCall::AssetConversion(pallet_asset_conversion::Call::create_pool {
		asset1: Box::new(asset1),
		asset2: Box::new(asset2),
	})
}

pub fn add_liquidity_call(
	asset1: NativeOrWithId<u32>,
	asset2: NativeOrWithId<u32>,
	who: u128,
) -> RuntimeCall {
	RuntimeCall::AssetConversion(pallet_asset_conversion::Call::add_liquidity {
		asset1: Box::new(asset1),
		asset2: Box::new(asset2),
		amount1_desired: 10,
		amount2_desired: 10,
		amount1_min: 1,
		amount2_min: 1,
		mint_to: who,
	})
}

pub fn contains_call(call: &RuntimeCall) -> bool {
	<AssetWaiting as Contains<RuntimeCall>>::contains(call)
}