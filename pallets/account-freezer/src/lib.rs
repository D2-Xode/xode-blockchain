//! # Xode Account Freezer Pallet
//!
//! This is free and unencumbered software released into the public domain.
#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub mod weights;
pub use weights::*;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_support::traits::fungible::{InspectFreeze, MutateFreeze};
	use frame_system::pallet_prelude::*;
	use sp_runtime::traits::{Bounded, Zero};

	pub type BalanceOf<T> = <<T as Config>::Currency as frame_support::traits::fungible::Inspect<
		<T as frame_system::Config>::AccountId,
	>>::Balance;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// The fungible implementation used to freeze/thaw the native (XON) balance.
		type Currency: MutateFreeze<Self::AccountId, Id = Self::RuntimeFreezeReason>
			+ InspectFreeze<Self::AccountId, Id = Self::RuntimeFreezeReason>;

		/// Overarching freeze reason; must be constructible from this pallet's own reason.
		type RuntimeFreezeReason: From<FreezeReason>;

		/// Origin allowed to freeze and thaw accounts.
		type FreezeOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	/// Freeze reason contributed by this pallet, folded into `RuntimeFreezeReason`.
	#[pallet::composite_enum]
	pub enum FreezeReason {
		AccountFrozen,
	}

	/// Accounts currently frozen by this pallet.
	#[pallet::storage]
	pub type Frozen<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, (), OptionQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// An account was frozen indefinitely.
		AccountFrozen { who: T::AccountId },
		/// A specific amount of an account's balance was frozen indefinitely.
		AmountFrozen { who: T::AccountId, amount: BalanceOf<T> },
		/// An account was thawed by `FreezeOrigin`.
		AccountThawed { who: T::AccountId },
		/// A specific amount was released from an account's freeze by `FreezeOrigin`.
		AmountThawed { who: T::AccountId, amount: BalanceOf<T> },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The account already has an active freeze.
		AlreadyFrozen,
		/// The account has no active freeze.
		NotFrozen,
		/// The requested thaw amount exceeds what's currently frozen.
		AmountExceedsFrozen,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Freeze `who`'s entire transferable native balance indefinitely.
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::freeze_account())]
		pub fn freeze_account(origin: OriginFor<T>, who: T::AccountId) -> DispatchResult {
			T::FreezeOrigin::ensure_origin(origin)?;
			ensure!(!Frozen::<T>::contains_key(&who), Error::<T>::AlreadyFrozen);

			T::Currency::set_freeze(&FreezeReason::AccountFrozen.into(), &who, BalanceOf::<T>::max_value())?;
			Frozen::<T>::insert(&who, ());

			Self::deposit_event(Event::AccountFrozen { who });
			Ok(())
		}

		/// Freeze `amount` of `who`'s transferable native balance indefinitely.
		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::freeze_amount())]
		pub fn freeze_amount(origin: OriginFor<T>, who: T::AccountId, amount: BalanceOf<T>) -> DispatchResult {
			T::FreezeOrigin::ensure_origin(origin)?;
			ensure!(!Frozen::<T>::contains_key(&who), Error::<T>::AlreadyFrozen);

			T::Currency::set_freeze(&FreezeReason::AccountFrozen.into(), &who, amount)?;
			Frozen::<T>::insert(&who, ());

			Self::deposit_event(Event::AmountFrozen { who, amount });
			Ok(())
		}

		/// Thaw a previously frozen account. Restricted to `FreezeOrigin`.
		#[pallet::call_index(2)]
		#[pallet::weight(<T as Config>::WeightInfo::thaw_account())]
		pub fn thaw_account(origin: OriginFor<T>, who: T::AccountId) -> DispatchResult {
			T::FreezeOrigin::ensure_origin(origin)?;
			ensure!(Frozen::<T>::contains_key(&who), Error::<T>::NotFrozen);
			T::Currency::thaw(&FreezeReason::AccountFrozen.into(), &who)?;
			Frozen::<T>::remove(&who);
			Self::deposit_event(Event::AccountThawed { who });
			Ok(())
		}

		/// Release `amount` from `who`'s freeze, leaving the rest frozen. If `amount` covers the
		/// entire frozen balance, the account is fully thawed. Restricted to `FreezeOrigin`.
		#[pallet::call_index(3)]
		#[pallet::weight(<T as Config>::WeightInfo::thaw_amount())]
		pub fn thaw_amount(origin: OriginFor<T>, who: T::AccountId, amount: BalanceOf<T>) -> DispatchResult {
			T::FreezeOrigin::ensure_origin(origin)?;
			ensure!(Frozen::<T>::contains_key(&who), Error::<T>::NotFrozen);

			let id = FreezeReason::AccountFrozen.into();
			let frozen = T::Currency::balance_frozen(&id, &who);
			ensure!(amount <= frozen, Error::<T>::AmountExceedsFrozen);

			let remaining = frozen - amount;
			if remaining.is_zero() {
				T::Currency::thaw(&id, &who)?;
				Frozen::<T>::remove(&who);
			} else {
				T::Currency::set_freeze(&id, &who, remaining)?;
			}

			Self::deposit_event(Event::AmountThawed { who, amount });
			Ok(())
		}
	}
}
