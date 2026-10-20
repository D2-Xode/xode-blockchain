#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use core::marker::PhantomData;

pub trait WeightInfo {
	fn vested_transfer() -> Weight;
	fn claim(i: u32, ) -> Weight;
	fn update_vesting_schedules(i: u32, ) -> Weight;
}

// Placeholders until benchmarked (see `benchmarking.rs`), set above `orml-vesting`'s defaults
// because asset transfers and freezes touch more storage than native locks.
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	// VestingAdmins::Members, Assets::Asset, VestingSchedules, Assets::Account (x2),
	// AssetsFreezer::Freezes, AssetsFreezer::FrozenBalances, System::Account.
	fn vested_transfer() -> Weight {
		Weight::from_parts(80_000_000, 8_000)
			.saturating_add(T::DbWeight::get().reads(8_u64))
			.saturating_add(T::DbWeight::get().writes(7_u64))
	}
	// VestingSchedules, AssetsFreezer::Freezes, AssetsFreezer::FrozenBalances.
	fn claim(i: u32, ) -> Weight {
		Weight::from_parts(35_000_000, 4_000)
			.saturating_add(Weight::from_parts(100_000, 0).saturating_mul(i as u64))
			.saturating_add(T::DbWeight::get().reads(3_u64))
			.saturating_add(T::DbWeight::get().writes(3_u64))
	}
	// Assets::Asset, Assets::Account, AssetsFreezer::Freezes, AssetsFreezer::FrozenBalances,
	// VestingSchedules.
	fn update_vesting_schedules(i: u32, ) -> Weight {
		Weight::from_parts(35_000_000, 4_000)
			.saturating_add(Weight::from_parts(150_000, 0).saturating_mul(i as u64))
			.saturating_add(T::DbWeight::get().reads(3_u64))
			.saturating_add(T::DbWeight::get().writes(3_u64))
	}
}

impl WeightInfo for () {
	fn vested_transfer() -> Weight {
		Weight::from_parts(80_000_000, 0)
			.saturating_add(RocksDbWeight::get().reads(8_u64))
			.saturating_add(RocksDbWeight::get().writes(7_u64))
	}
	fn claim(i: u32, ) -> Weight {
		Weight::from_parts(35_000_000, 0)
			.saturating_add(Weight::from_parts(100_000, 0).saturating_mul(i as u64))
			.saturating_add(RocksDbWeight::get().reads(3_u64))
			.saturating_add(RocksDbWeight::get().writes(3_u64))
	}
	fn update_vesting_schedules(i: u32, ) -> Weight {
		Weight::from_parts(35_000_000, 0)
			.saturating_add(Weight::from_parts(150_000, 0).saturating_mul(i as u64))
			.saturating_add(RocksDbWeight::get().reads(3_u64))
			.saturating_add(RocksDbWeight::get().writes(3_u64))
	}
}
