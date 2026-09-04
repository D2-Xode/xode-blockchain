//! # Xode Freezer Pallet
//!
//! This is free and unencumbered software released into the public domain.
//!
//! Anyone is free to copy, modify, publish, use, compile, sell, or
//! distribute this software, either in source code form or as a compiled
//! binary, for any purpose, commercial or non-commercial, and by any
//! means.
//!
//! In jurisdictions that recognize copyright laws, the author or authors
//! of this software dedicate any and all copyright interest in the
//! software to the public domain. We make this dedication for the benefit
//! of the public at large and to the detriment of our heirs and
//! successors. We intend this dedication to be an overt act of
//! relinquishment in perpetuity of all present and future rights to this
//! software under copyright law.
//!
//! THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
//! EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
//! MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.
//! IN NO EVENT SHALL THE AUTHORS BE LIABLE FOR ANY CLAIM, DAMAGES OR
//! OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE,
//! ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR
//! OTHER DEALINGS IN THE SOFTWARE.
//!
//! For more information, please refer to <http://unlicense.org>
#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub mod weights;
pub use weights::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::{
		pallet_prelude::*,
		traits::fungible::{Inspect, MutateFreeze},
	};
	use frame_system::pallet_prelude::*;

	pub type BalanceOf<T> =
		<<T as Config>::Currency as Inspect<<T as frame_system::Config>::AccountId>>::Balance;

	/// Reason this pallet freezes a native balance, namespaced into `RuntimeFreezeReason`.
	#[pallet::composite_enum]
	pub enum FreezeReason {
		/// Balance frozen by a Technical Committee decision.
		TechnicalCommitteeFreeze,
	}

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// The runtime's aggregated freeze reason, used to namespace this pallet's freezes
		/// against those of other pallets sharing the same `Currency`.
		type RuntimeFreezeReason: From<FreezeReason>;

		/// The native currency that balances are frozen on.
		type Currency: Inspect<Self::AccountId>
			+ MutateFreeze<Self::AccountId, Id = Self::RuntimeFreezeReason>;

		/// The origin allowed to freeze and thaw a native balance (the Technical Committee).
		type FreezeOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// A type representing the weights required by the dispatchables of this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	/// Accounts currently frozen by this pallet, and the amount they are frozen for.
	#[pallet::storage]
	pub type FrozenAccounts<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, BalanceOf<T>, OptionQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A native balance was frozen for an account.
		BalanceFrozen { who: T::AccountId, amount: BalanceOf<T> },
		/// A native balance freeze was lifted for an account.
		BalanceThawed { who: T::AccountId },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The account has no active freeze from this pallet.
		NotFrozen,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Freeze `amount` of `who`'s native balance so it becomes unspendable, until thawed.
		///
		/// Calling this again on an already-frozen account replaces the previously frozen
		/// amount rather than adding to it. Only callable by `FreezeOrigin` (the Technical
		/// Committee).
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::freeze())]
		pub fn freeze(origin: OriginFor<T>, who: T::AccountId, amount: BalanceOf<T>) -> DispatchResult {
			T::FreezeOrigin::ensure_origin(origin)?;

			T::Currency::set_freeze(&FreezeReason::TechnicalCommitteeFreeze.into(), &who, amount)?;
			FrozenAccounts::<T>::insert(&who, amount);

			Self::deposit_event(Event::BalanceFrozen { who, amount });
			Ok(())
		}

		/// Lift a freeze previously placed on `who`'s native balance by this pallet. Only
		/// callable by `FreezeOrigin` (the Technical Committee).
		#[pallet::call_index(1)]
		#[pallet::weight(T::WeightInfo::thaw())]
		pub fn thaw(origin: OriginFor<T>, who: T::AccountId) -> DispatchResult {
			T::FreezeOrigin::ensure_origin(origin)?;

			ensure!(FrozenAccounts::<T>::contains_key(&who), Error::<T>::NotFrozen);

			T::Currency::thaw(&FreezeReason::TechnicalCommitteeFreeze.into(), &who)?;
			FrozenAccounts::<T>::remove(&who);

			Self::deposit_event(Event::BalanceThawed { who });
			Ok(())
		}
	}
}
