use super::*;
use crate::Pallet as AccountFreezer;
use frame_benchmarking::v2::*;
use frame_support::traits::fungible::{Inspect, Mutate};
use frame_system::RawOrigin;
use sp_runtime::{traits::Bounded, Saturating};

fn funded_caller<T: Config>() -> (T::AccountId, BalanceOf<T>)
where
	T::Currency: Mutate<T::AccountId>,
{
	let who: T::AccountId = whitelisted_caller();
	let ed = T::Currency::minimum_balance();
	T::Currency::set_balance(&who, ed.saturating_mul(100u32.into()).saturating_add(T::LockFeeReserve::get()));
	(who, ed.saturating_mul(10u32.into()))
}

#[benchmarks(where T::Currency: Mutate<T::AccountId>)]
mod benchmarks {
	use super::*;

	#[benchmark]
	fn freeze_account() {
		let who: T::AccountId = whitelisted_caller();
		#[extrinsic_call]
		_(RawOrigin::Root, who);
	}

	#[benchmark]
	fn freeze_amount() {
		let who: T::AccountId = whitelisted_caller();
		let amount = BalanceOf::<T>::max_value();
		#[extrinsic_call]
		_(RawOrigin::Root, who, amount);
	}

	#[benchmark]
	fn thaw_account() {
		let who: T::AccountId = whitelisted_caller();
		AccountFreezer::<T>::freeze_account(RawOrigin::Root.into(), who.clone()).unwrap();
		#[extrinsic_call]
		_(RawOrigin::Root, who);
	}

	#[benchmark]
	fn thaw_amount() {
		let who: T::AccountId = whitelisted_caller();
		let amount = BalanceOf::<T>::max_value();
		AccountFreezer::<T>::freeze_amount(RawOrigin::Root.into(), who.clone(), amount).unwrap();
		#[extrinsic_call]
		_(RawOrigin::Root, who, amount);
	}

	#[benchmark]
	fn lock_self() {
		let (who, amount) = funded_caller::<T>();
		// worst case: adding to an existing, still-active lock
		AccountFreezer::<T>::lock_self(RawOrigin::Signed(who.clone()).into(), amount, T::MinLockDuration::get()).unwrap();
		#[extrinsic_call]
		_(RawOrigin::Signed(who), amount, T::MaxLockDuration::get());
	}

	#[benchmark]
	fn unlock_expired() {
		let (who, amount) = funded_caller::<T>();
		let duration = T::MinLockDuration::get();
		AccountFreezer::<T>::lock_self(RawOrigin::Signed(who.clone()).into(), amount, duration).unwrap();
		let now = frame_system::Pallet::<T>::block_number();
		frame_system::Pallet::<T>::set_block_number(now.saturating_add(duration));
		#[extrinsic_call]
		_(RawOrigin::Signed(who.clone()), who.clone());
	}

	impl_benchmark_test_suite!(AccountFreezer, crate::mock::new_test_ext(), crate::mock::Test);
}
