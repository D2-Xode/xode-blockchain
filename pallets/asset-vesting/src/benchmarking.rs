use super::*;
use crate::Pallet as AssetVesting;
use alloc::vec;
use frame_benchmarking::v2::*;
use frame_support::traits::{
	fungibles::{Create, Mutate},
	tokens::Preservation,
	EnsureOrigin, Get,
};
use frame_system::RawOrigin;
use sp_runtime::{
	traits::{BlockNumberProvider, StaticLookup},
	Saturating,
};

/// Creates a sufficient asset owned by a caller that isn't an admin, funded for `count`
/// schedules. Vesting as the owner is the slower origin path: the admin check fails first.
fn setup<T: Config>(count: u32) -> (T::AccountId, AssetIdOf<T>, VestingScheduleOf<T>)
where
	T::Assets: Create<T::AccountId>,
{
	let owner: T::AccountId = whitelisted_caller();
	let asset_id = T::BenchmarkHelper::asset_id();
	T::Assets::create(asset_id.clone(), owner.clone(), true, 1u32.into()).unwrap();

	let schedule = VestingScheduleOf::<T> {
		start: T::BlockNumberProvider::current_block_number().saturating_add(1u32.into()),
		period: 2u32.into(),
		period_count: 3,
		per_period: 1_000u32.into(),
	};
	let funds = schedule.total_amount().unwrap().saturating_mul(count.into());
	T::Assets::mint_into(asset_id.clone(), &owner, funds).unwrap();
	(owner, asset_id, schedule)
}

#[benchmarks(where T::Assets: Create<T::AccountId>)]
mod benchmarks {
	use super::*;

	#[benchmark]
	fn vested_transfer() {
		let max = T::MaxVestingSchedules::get();
		let (owner, asset_id, schedule) = setup::<T>(max);
		let to: T::AccountId = account("to", 0, 0);
		let to_lookup = T::Lookup::unlookup(to.clone());
		// worst case: `to` already has all but one schedule, and each is re-checked
		for _ in 1..max {
			AssetVesting::<T>::vested_transfer(
				RawOrigin::Signed(owner.clone()).into(),
				asset_id.clone(),
				to_lookup.clone(),
				schedule.clone(),
			)
			.unwrap();
		}

		#[extrinsic_call]
		_(RawOrigin::Signed(owner), asset_id.clone(), to_lookup, schedule);

		assert_eq!(VestingSchedules::<T>::decode_len(&to, &asset_id), Some(max as usize));
	}

	#[benchmark]
	fn claim(i: Linear<1, { T::MaxVestingSchedules::get() }>) {
		let (owner, asset_id, schedule) = setup::<T>(i);
		let to: T::AccountId = account("to", 0, 0);
		let to_lookup = T::Lookup::unlookup(to.clone());
		for _ in 0..i {
			AssetVesting::<T>::vested_transfer(
				RawOrigin::Signed(owner.clone()).into(),
				asset_id.clone(),
				to_lookup.clone(),
				schedule.clone(),
			)
			.unwrap();
		}
		// one period in: every schedule stays and is written back, and the freeze is lowered
		T::BlockNumberProvider::set_block_number(schedule.start.saturating_add(schedule.period));

		#[extrinsic_call]
		_(RawOrigin::Signed(to.clone()), asset_id.clone());

		assert_eq!(VestingSchedules::<T>::decode_len(&to, &asset_id), Some(i as usize));
	}

	#[benchmark]
	fn update_vesting_schedules(i: Linear<1, { T::MaxVestingSchedules::get() }>) -> Result<(), BenchmarkError> {
		let origin = T::ForceOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;
		let (owner, asset_id, schedule) = setup::<T>(i);
		let to: T::AccountId = account("to", 0, 0);
		let funds = schedule.total_amount().unwrap().saturating_mul(i.into());
		T::Assets::transfer(asset_id.clone(), &owner, &to, funds, Preservation::Expendable).unwrap();
		let schedules = vec![schedule; i as usize];

		#[extrinsic_call]
		_(origin as T::RuntimeOrigin, asset_id.clone(), T::Lookup::unlookup(to.clone()), schedules);

		assert_eq!(VestingSchedules::<T>::decode_len(&to, &asset_id), Some(i as usize));
		Ok(())
	}

	impl_benchmark_test_suite!(AssetVesting, crate::mock::new_test_ext(), crate::mock::Test);
}
