use frame_support::{
	derive_impl,
	instances::Instance1,
	traits::{AsEnsureOriginWithArg, ConstU128, ConstU32, SortedMembers, VariantCountOf},
};
use frame_system::{EnsureRoot, EnsureSigned, EnsureSignedBy};
use sp_runtime::{traits::IdentityLookup, BuildStorage};

pub type AccountId = u64;
pub type Balance = u128;
pub type AssetId = u32;

/// A vesting admin.
pub const ADMIN: AccountId = 1;
/// Owner of `ASSET`.
pub const OWNER: AccountId = 2;
/// Owner of `OTHER_ASSET`.
pub const OTHER_OWNER: AccountId = 3;
pub const ALICE: AccountId = 4;
pub const BOB: AccountId = 5;
/// Holds no XON, so it can't receive a non-sufficient asset.
pub const NO_XON: AccountId = 6;

/// Non-sufficient asset owned by `OWNER`. `ADMIN` and `OWNER` each hold `INITIAL` of it.
pub const ASSET: AssetId = 1;
/// Sufficient asset owned by `OTHER_OWNER`, who holds `INITIAL` of it.
pub const OTHER_ASSET: AssetId = 2;
pub const MIN_BALANCE: Balance = 10;
pub const INITIAL: Balance = 1_000_000;

#[frame_support::runtime]
mod test_runtime {
	#[runtime::runtime]
	#[runtime::derive(
		RuntimeCall, RuntimeEvent, RuntimeError, RuntimeOrigin,
		RuntimeFreezeReason, RuntimeHoldReason, RuntimeSlashReason, RuntimeLockId, RuntimeTask
	)]
	pub struct Test;

	#[runtime::pallet_index(0)]
	pub type System = frame_system;
	#[runtime::pallet_index(1)]
	pub type Balances = pallet_balances;
	#[runtime::pallet_index(2)]
	pub type Assets = pallet_assets::Pallet<Test, Instance1>;
	#[runtime::pallet_index(3)]
	pub type AssetsFreezer = pallet_assets_freezer::Pallet<Test, Instance1>;
	#[runtime::pallet_index(4)]
	pub type AssetVesting = crate;
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
	type Block = frame_system::mocking::MockBlock<Test>;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type AccountData = pallet_balances::AccountData<Balance>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
	type Balance = Balance;
	type ExistentialDeposit = ConstU128<1>;
	type AccountStore = System;
	type RuntimeFreezeReason = RuntimeFreezeReason;
	type FreezeIdentifier = RuntimeFreezeReason;
	type MaxFreezes = VariantCountOf<RuntimeFreezeReason>;
}

impl pallet_assets::Config<Instance1> for Test {
	type RuntimeEvent = RuntimeEvent;
	type Balance = Balance;
	type RemoveItemsLimit = ConstU32<1000>;
	type AssetId = AssetId;
	type AssetIdParameter = AssetId;
	type ReserveData = ();
	type Currency = Balances;
	type CreateOrigin = AsEnsureOriginWithArg<EnsureSigned<Self::AccountId>>;
	type ForceOrigin = EnsureRoot<Self::AccountId>;
	type AssetDeposit = ConstU128<1>;
	type AssetAccountDeposit = ConstU128<1>;
	type MetadataDepositBase = ConstU128<1>;
	type MetadataDepositPerByte = ConstU128<1>;
	type ApprovalDeposit = ConstU128<1>;
	type StringLimit = ConstU32<50>;
	type Holder = ();
	type Freezer = AssetsFreezer;
	type Extra = ();
	type WeightInfo = ();
	type CallbackHandle = ();
	pallet_assets::runtime_benchmarks_enabled! {
		type BenchmarkHelper = ();
	}
}

impl pallet_assets_freezer::Config<Instance1> for Test {
	type RuntimeFreezeReason = RuntimeFreezeReason;
	type RuntimeEvent = RuntimeEvent;
}

pub struct VestingAdmins;
impl SortedMembers<AccountId> for VestingAdmins {
	fn sorted_members() -> Vec<AccountId> {
		vec![ADMIN]
	}
}

#[cfg(feature = "runtime-benchmarks")]
pub struct BenchmarkHelper;
#[cfg(feature = "runtime-benchmarks")]
impl crate::BenchmarkHelper<AssetId> for BenchmarkHelper {
	fn asset_id() -> AssetId {
		100
	}
}

impl crate::Config for Test {
	type Assets = Assets;
	type Freezer = AssetsFreezer;
	type RuntimeFreezeReason = RuntimeFreezeReason;
	type AdminOrigin = EnsureSignedBy<VestingAdmins, AccountId>;
	type ForceOrigin = EnsureRoot<AccountId>;
	type MaxVestingSchedules = ConstU32<3>;
	type BlockNumberProvider = System;
	type WeightInfo = ();
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = BenchmarkHelper;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();
	pallet_balances::GenesisConfig::<Test> {
		balances: vec![(ADMIN, 1_000), (OWNER, 1_000), (OTHER_OWNER, 1_000), (ALICE, 1_000), (BOB, 1_000)],
		..Default::default()
	}
	.assimilate_storage(&mut t)
	.unwrap();
	pallet_assets::GenesisConfig::<Test, Instance1> {
		assets: vec![(ASSET, OWNER, false, MIN_BALANCE), (OTHER_ASSET, OTHER_OWNER, true, MIN_BALANCE)],
		metadata: vec![],
		accounts: vec![(ASSET, ADMIN, INITIAL), (ASSET, OWNER, INITIAL), (OTHER_ASSET, OTHER_OWNER, INITIAL)],
		next_asset_id: None,
		reserves: vec![],
	}
	.assimilate_storage(&mut t)
	.unwrap();
	let mut ext: sp_io::TestExternalities = t.into();
	ext.execute_with(|| System::set_block_number(1));
	ext
}
