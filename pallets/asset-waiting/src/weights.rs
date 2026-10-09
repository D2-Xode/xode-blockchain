use frame_support::traits::Get;
use frame_support::weights::Weight;
use pallet_asset_conversion::WeightInfo as AssetConversionWeightInfo;

pub trait WeightInfo {
	fn create_pool() -> Weight;
	fn approve_pool() -> Weight;
	fn reject_pool() -> Weight;
}

pub struct SubstrateWeight<T>(core::marker::PhantomData<T>);

impl<T> WeightInfo for SubstrateWeight<T>
where
	T: crate::pallet::Config,
{
	fn create_pool() -> Weight {
		<T as pallet_asset_conversion::Config>::WeightInfo::create_pool()
			.saturating_add(T::DbWeight::get().reads_writes(1_u64, 1_u64))
	}

	fn approve_pool() -> Weight {
		T::DbWeight::get().reads_writes(1_u64, 1_u64)
	}

	fn reject_pool() -> Weight {
		T::DbWeight::get().reads_writes(2_u64, 2_u64)
	}
}

impl WeightInfo for () {
	fn create_pool() -> Weight {
		Weight::zero()
	}

	fn approve_pool() -> Weight {
		Weight::zero()
	}

	fn reject_pool() -> Weight {
		Weight::zero()
	}
}