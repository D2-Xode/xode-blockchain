//! Weights for pallet_xode_freezer.
//!
//! NOTE: these are placeholder estimates, not measured via `frame-benchmarking`.
//! Run the standard benchmarking CLI against this pallet before relying on them
//! for production fee/weight accounting.

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::Weight};
use core::marker::PhantomData;

/// Weight functions needed for pallet_xode_freezer.
pub trait WeightInfo {
	fn freeze() -> Weight;
	fn thaw() -> Weight;
}

/// Weights for pallet_xode_freezer using the Substrate node and recommended hardware.
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	fn freeze() -> Weight {
		Weight::from_parts(20_000_000, 3593)
			.saturating_add(T::DbWeight::get().reads(1_u64))
			.saturating_add(T::DbWeight::get().writes(2_u64))
	}
	fn thaw() -> Weight {
		Weight::from_parts(20_000_000, 3593)
			.saturating_add(T::DbWeight::get().reads(1_u64))
			.saturating_add(T::DbWeight::get().writes(2_u64))
	}
}

impl WeightInfo for () {
	fn freeze() -> Weight {
		Weight::from_parts(20_000_000, 0)
	}
	fn thaw() -> Weight {
		Weight::from_parts(20_000_000, 0)
	}
}
