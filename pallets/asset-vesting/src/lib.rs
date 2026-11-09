//! # Xode Asset Vesting Pallet
//!
//! Graded vesting for local assets (`pallet_assets`). It uses the same schedule as `Vesting`
//! (`orml_vesting::VestingSchedule`): from block `start`, `per_period` unlocks every `period`
//! blocks, `period_count` times.
//!
//! `Vesting` locks native XON. This pallet freezes part of one asset balance instead (through
//! `pallet_assets_freezer`, reason `FreezeReason::Vesting`). Schedules are stored per
//! (account, asset), so each asset vests on its own.
//!
//! Only two kinds of caller can create a schedule:
//! - `AdminOrigin` (the `VestingAdmins` in the Xode runtime), for any local asset;
//! - the asset's owner, for that asset only.
//!
//! The vested tokens are taken from the caller's own balance of the asset.
//!
//! This is free and unencumbered software released into the public domain.
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub use orml_vesting::VestingSchedule;
pub use pallet::*;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub mod weights;
pub use weights::*;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

/// Benchmark setup that depends on the runtime.
#[cfg(feature = "runtime-benchmarks")]
pub trait BenchmarkHelper<AssetId> {
	/// An asset id that `fungibles::Create::create` will accept for a new asset.
	fn asset_id() -> AssetId;
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use alloc::vec::Vec;
	use frame_support::{
		pallet_prelude::*,
		traits::{
			fungibles::{roles::Inspect as InspectRoles, Inspect, InspectFreeze, Mutate, MutateFreeze},
			tokens::Preservation,
		},
	};
	use frame_system::pallet_prelude::*;
	use sp_runtime::{
		traits::{BlockNumberProvider, CheckedAdd, Saturating, StaticLookup, Zero},
		ArithmeticError,
	};

	pub type AssetIdOf<T> =
		<<T as Config>::Assets as Inspect<<T as frame_system::Config>::AccountId>>::AssetId;
	pub type BalanceOf<T> =
		<<T as Config>::Assets as Inspect<<T as frame_system::Config>::AccountId>>::Balance;
	pub type VestingScheduleOf<T> = VestingSchedule<BlockNumberFor<T>, BalanceOf<T>>;
	type AccountIdLookupOf<T> = <<T as frame_system::Config>::Lookup as StaticLookup>::Source;

	#[pallet::config]
	pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
		/// The local assets. Moves the vested tokens and says who owns each asset.
		type Assets: Mutate<Self::AccountId> + InspectRoles<Self::AccountId>;

		/// Freezes the unvested part of an asset balance (`pallet_assets_freezer` for `Assets`).
		type Freezer: MutateFreeze<
				Self::AccountId,
				AssetId = AssetIdOf<Self>,
				Balance = BalanceOf<Self>,
				Id = Self::RuntimeFreezeReason,
			> + InspectFreeze<Self::AccountId, Id = Self::RuntimeFreezeReason>;

		/// Overarching freeze reason; must be constructible from this pallet's own reason.
		type RuntimeFreezeReason: From<FreezeReason>;

		/// Origin that may vest any asset. Returns the account the vested tokens come from.
		/// A signed caller that fails this check may still vest an asset it owns.
		type AdminOrigin: EnsureOrigin<Self::RuntimeOrigin, Success = Self::AccountId>;

		/// Origin that may replace an account's schedules for an asset.
		type ForceOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Maximum schedules per account, per asset.
		#[pallet::constant]
		type MaxVestingSchedules: Get<u32>;

		/// The clock that schedule `start` and `period` are measured in.
		type BlockNumberProvider: BlockNumberProvider<BlockNumber = BlockNumberFor<Self>>;

		type WeightInfo: WeightInfo;

		#[cfg(feature = "runtime-benchmarks")]
		type BenchmarkHelper: BenchmarkHelper<AssetIdOf<Self>>;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	/// Freeze reasons contributed by this pallet, folded into `RuntimeFreezeReason`.
	#[pallet::composite_enum]
	pub enum FreezeReason {
		/// Asset balance that hasn't vested yet.
		Vesting,
	}

	/// Vesting schedules of an account for one asset.
	#[pallet::storage]
	pub type VestingSchedules<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		AssetIdOf<T>,
		BoundedVec<VestingScheduleOf<T>, T::MaxVestingSchedules>,
		ValueQuery,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A vesting schedule was added to `to`, funded by `from`.
		VestingScheduleAdded {
			asset_id: AssetIdOf<T>,
			from: T::AccountId,
			to: T::AccountId,
			vesting_schedule: VestingScheduleOf<T>,
		},
		/// Vested tokens were unfrozen. `unlocked` was released now; `remaining` is still frozen.
		Claimed { asset_id: AssetIdOf<T>, who: T::AccountId, unlocked: BalanceOf<T>, remaining: BalanceOf<T> },
		/// `ForceOrigin` replaced an account's schedules for an asset.
		VestingSchedulesUpdated { asset_id: AssetIdOf<T>, who: T::AccountId },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The caller is neither `AdminOrigin` nor the asset's owner.
		NotAdminOrAssetOwner,
		/// Vesting period is zero.
		ZeroVestingPeriod,
		/// Number of vests is zero.
		ZeroVestingPeriodCount,
		/// The schedule's total is below the asset's minimum balance.
		AmountLow,
		/// The account's asset balance can't cover the amount to freeze.
		InsufficientBalanceToLock,
		/// The account already has `MaxVestingSchedules` schedules for this asset.
		MaxVestingSchedulesExceeded,
		/// The account has no vesting schedules for this asset.
		NotVesting,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Unfreeze the caller's vested balance of `asset_id`.
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::claim(T::MaxVestingSchedules::get()))]
		pub fn claim(origin: OriginFor<T>, asset_id: AssetIdOf<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Self::do_claim(asset_id, &who)
		}

		/// Move `schedule`'s total of `asset_id` from the caller to `dest` and freeze it there,
		/// unfreezing per the schedule. Callable by `AdminOrigin` or the asset's owner.
		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::vested_transfer())]
		pub fn vested_transfer(
			origin: OriginFor<T>,
			asset_id: AssetIdOf<T>,
			dest: AccountIdLookupOf<T>,
			schedule: VestingScheduleOf<T>,
		) -> DispatchResult {
			let from = Self::ensure_admin_or_owner(origin, asset_id.clone())?;
			let to = T::Lookup::lookup(dest)?;

			Self::do_vested_transfer(asset_id.clone(), &from, &to, schedule.clone())?;

			Self::deposit_event(Event::VestingScheduleAdded { asset_id, from, to, vesting_schedule: schedule });
			Ok(())
		}

		/// Replace `who`'s schedules for `asset_id`; an empty list removes them and unfreezes
		/// everything. Restricted to `ForceOrigin`. Moves no tokens.
		#[pallet::call_index(2)]
		#[pallet::weight(<T as Config>::WeightInfo::update_vesting_schedules(vesting_schedules.len() as u32))]
		pub fn update_vesting_schedules(
			origin: OriginFor<T>,
			asset_id: AssetIdOf<T>,
			who: AccountIdLookupOf<T>,
			vesting_schedules: Vec<VestingScheduleOf<T>>,
		) -> DispatchResult {
			T::ForceOrigin::ensure_origin(origin)?;
			let who = T::Lookup::lookup(who)?;

			Self::do_update_vesting_schedules(asset_id.clone(), &who, vesting_schedules)?;

			Self::deposit_event(Event::VestingSchedulesUpdated { asset_id, who });
			Ok(())
		}

		/// Permissionless: unfreeze `dest`'s vested balance of `asset_id`.
		#[pallet::call_index(3)]
		#[pallet::weight(<T as Config>::WeightInfo::claim(T::MaxVestingSchedules::get()))]
		pub fn claim_for(origin: OriginFor<T>, asset_id: AssetIdOf<T>, dest: AccountIdLookupOf<T>) -> DispatchResult {
			ensure_signed(origin)?;
			let who = T::Lookup::lookup(dest)?;
			Self::do_claim(asset_id, &who)
		}
	}

	impl<T: Config> Pallet<T> {
		/// The account vested tokens come from: an `AdminOrigin` caller, or the asset's owner.
		fn ensure_admin_or_owner(origin: OriginFor<T>, asset_id: AssetIdOf<T>) -> Result<T::AccountId, DispatchError> {
			match T::AdminOrigin::try_origin(origin) {
				Ok(admin) => Ok(admin),
				Err(origin) => {
					let who = ensure_signed(origin)?;
					ensure!(T::Assets::owner(asset_id).as_ref() == Some(&who), Error::<T>::NotAdminOrAssetOwner);
					Ok(who)
				},
			}
		}

		fn do_claim(asset_id: AssetIdOf<T>, who: &T::AccountId) -> DispatchResult {
			ensure!(VestingSchedules::<T>::contains_key(who, &asset_id), Error::<T>::NotVesting);

			let id = FreezeReason::Vesting.into();
			let frozen = T::Freezer::balance_frozen(asset_id.clone(), &id, who);
			let remaining = Self::locked_balance(asset_id.clone(), who);
			if remaining.is_zero() {
				T::Freezer::thaw(asset_id.clone(), &id, who)?;
			} else {
				T::Freezer::set_freeze(asset_id.clone(), &id, who, remaining)?;
			}

			Self::deposit_event(Event::Claimed {
				asset_id,
				who: who.clone(),
				unlocked: frozen.saturating_sub(remaining),
				remaining,
			});
			Ok(())
		}

		/// Returns the amount still vesting at the current block, dropping finished schedules
		/// (and the storage entry, once none are left).
		fn locked_balance(asset_id: AssetIdOf<T>, who: &T::AccountId) -> BalanceOf<T> {
			let now = T::BlockNumberProvider::current_block_number();
			VestingSchedules::<T>::mutate_exists(who, asset_id, |maybe_schedules| {
				let total = if let Some(schedules) = maybe_schedules.as_mut() {
					let mut total: BalanceOf<T> = Zero::zero();
					schedules.retain(|s| {
						let amount = s.locked_amount(now);
						total = total.saturating_add(amount);
						!amount.is_zero()
					});
					total
				} else {
					Zero::zero()
				};
				if total.is_zero() {
					*maybe_schedules = None;
				}
				total
			})
		}

		fn do_vested_transfer(
			asset_id: AssetIdOf<T>,
			from: &T::AccountId,
			to: &T::AccountId,
			schedule: VestingScheduleOf<T>,
		) -> DispatchResult {
			let schedule_amount = Self::ensure_valid_schedule(asset_id.clone(), &schedule)?;
			let total = Self::locked_balance(asset_id.clone(), to)
				.checked_add(&schedule_amount)
				.ok_or(ArithmeticError::Overflow)?;
			// Checked before any transfer, after `locked_balance` has dropped finished schedules.
			ensure!(
				VestingSchedules::<T>::decode_len(to, &asset_id).unwrap_or(0) < T::MaxVestingSchedules::get() as usize,
				Error::<T>::MaxVestingSchedulesExceeded
			);

			T::Assets::transfer(asset_id.clone(), from, to, schedule_amount, Preservation::Expendable)?;
			// `set_freeze` doesn't check the balance. This also covers `from == to`, where the
			// transfer above moved nothing.
			ensure!(T::Assets::balance(asset_id.clone(), to) >= total, Error::<T>::InsufficientBalanceToLock);

			T::Freezer::set_freeze(asset_id.clone(), &FreezeReason::Vesting.into(), to, total)?;
			VestingSchedules::<T>::try_append(to, asset_id, schedule)
				.map_err(|_| Error::<T>::MaxVestingSchedulesExceeded)?;
			Ok(())
		}

		fn do_update_vesting_schedules(
			asset_id: AssetIdOf<T>,
			who: &T::AccountId,
			schedules: Vec<VestingScheduleOf<T>>,
		) -> DispatchResult {
			let schedules: BoundedVec<VestingScheduleOf<T>, T::MaxVestingSchedules> =
				schedules.try_into().map_err(|_| Error::<T>::MaxVestingSchedulesExceeded)?;
			let id = FreezeReason::Vesting.into();

			if schedules.is_empty() {
				VestingSchedules::<T>::remove(who, &asset_id);
				return T::Freezer::thaw(asset_id, &id, who);
			}

			let total = schedules.iter().try_fold(BalanceOf::<T>::zero(), |acc, s| {
				let amount = Self::ensure_valid_schedule(asset_id.clone(), s)?;
				acc.checked_add(&amount).ok_or(DispatchError::from(ArithmeticError::Overflow))
			})?;
			ensure!(T::Assets::balance(asset_id.clone(), who) >= total, Error::<T>::InsufficientBalanceToLock);

			T::Freezer::set_freeze(asset_id.clone(), &id, who, total)?;
			VestingSchedules::<T>::insert(who, asset_id, schedules);
			Ok(())
		}

		/// Returns the schedule's total, or an error if the schedule is invalid.
		fn ensure_valid_schedule(
			asset_id: AssetIdOf<T>,
			schedule: &VestingScheduleOf<T>,
		) -> Result<BalanceOf<T>, DispatchError> {
			ensure!(!schedule.period.is_zero(), Error::<T>::ZeroVestingPeriod);
			ensure!(!schedule.period_count.is_zero(), Error::<T>::ZeroVestingPeriodCount);
			ensure!(schedule.end().is_some(), ArithmeticError::Overflow);

			let total = schedule.total_amount().ok_or(ArithmeticError::Overflow)?;
			// A zero total would leave `locked_amount` nothing to release; it's below any asset's
			// minimum balance, and unknown assets report a minimum of zero.
			ensure!(!total.is_zero() && total >= T::Assets::minimum_balance(asset_id), Error::<T>::AmountLow);
			Ok(total)
		}
	}
}
