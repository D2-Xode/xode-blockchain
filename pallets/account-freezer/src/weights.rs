#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use core::marker::PhantomData;

pub trait WeightInfo {
	fn freeze_account() -> Weight;
	fn freeze_amount() -> Weight;
	fn thaw_account() -> Weight;
	fn thaw_amount() -> Weight;
}

pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	fn freeze_account() -> Weight {
		Weight::from_parts(10_000_000, 3593)
			.saturating_add(T::DbWeight::get().reads(1_u64))
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
	fn freeze_amount() -> Weight {
		Weight::from_parts(10_000_000, 3593)
			.saturating_add(T::DbWeight::get().reads(1_u64))
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
	fn thaw_account() -> Weight {
		Weight::from_parts(10_000_000, 3593)
			.saturating_add(T::DbWeight::get().reads(1_u64))
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
	fn thaw_amount() -> Weight {
		Weight::from_parts(10_000_000, 3593)
			.saturating_add(T::DbWeight::get().reads(1_u64))
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
}

impl WeightInfo for () {
	fn freeze_account() -> Weight { Weight::from_parts(10_000_000, 0) }
	fn freeze_amount() -> Weight { Weight::from_parts(10_000_000, 0) }
	fn thaw_account() -> Weight { Weight::from_parts(10_000_000, 0) }
	fn thaw_amount() -> Weight { Weight::from_parts(10_000_000, 0) }
}
