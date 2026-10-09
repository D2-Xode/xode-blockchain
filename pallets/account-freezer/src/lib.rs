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
	use frame_support::traits::fungible::{Inspect, InspectFreeze, MutateFreeze};
	use frame_system::pallet_prelude::*;
	use sp_runtime::traits::{Bounded, Saturating, Zero};

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

		/// Shortest duration (in blocks) accepted by `lock_self`.
		#[pallet::constant]
		type MinLockDuration: Get<BlockNumberFor<Self>>;

		/// Longest duration (in blocks) accepted by `lock_self`.
		#[pallet::constant]
		type MaxLockDuration: Get<BlockNumberFor<Self>>;

		/// Free balance `lock_self` always leaves unlocked, so the owner can still pay fees
		/// (e.g. for `unlock_expired`).
		#[pallet::constant]
		type LockFeeReserve: Get<BalanceOf<Self>>;

		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	/// Freeze reasons contributed by this pallet, folded into `RuntimeFreezeReason`.
	/// Each reason holds its own amount; the account's frozen balance is the largest of them.
	#[pallet::composite_enum]
	pub enum FreezeReason {
		/// Admin / compliance freeze, set by `FreezeOrigin`.
		AccountFrozen,
		/// The owner's own time lock, set by `lock_self`.
		SelfLocked,
	}

	/// An account's own time lock: `amount` stays frozen until block `until`.
	#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub struct SelfLock<Balance, BlockNumber> {
		pub amount: Balance,
		pub until: BlockNumber,
	}

	/// Accounts currently frozen by this pallet.
	#[pallet::storage]
	pub type Frozen<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, (), OptionQuery>;

	/// Self-locks, one per account.
	#[pallet::storage]
	pub type SelfLocks<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		SelfLock<BalanceOf<T>, BlockNumberFor<T>>,
		OptionQuery,
	>;

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
		/// An account locked its own balance; `amount` is the lock's new total.
		SelfLocked { who: T::AccountId, amount: BalanceOf<T>, until: BlockNumberFor<T> },
		/// An expired self-lock was released.
		SelfUnlocked { who: T::AccountId, amount: BalanceOf<T> },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The account already has an active freeze.
		AlreadyFrozen,
		/// The account has no active freeze.
		NotFrozen,
		/// The requested thaw amount exceeds what's currently frozen.
		AmountExceedsFrozen,
		/// `lock_self` was called with a zero amount.
		ZeroAmount,
		/// Requested duration is below `MinLockDuration`.
		DurationTooShort,
		/// Requested duration exceeds `MaxLockDuration`.
		DurationTooLong,
		/// The lock's total plus `LockFeeReserve` would exceed the account's free balance.
		InsufficientBalance,
		/// The account has no self-lock.
		NotLocked,
		/// `unlock_expired` was called before the lock's expiry block.
		NotYetExpired,
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

		/// Freeze `amount` more of the caller's own native balance until at least now + `duration`.
		/// Adding to an active lock sums the amounts, and the unlock block never moves earlier.
		/// An expired lock that was never released is replaced, not added to.
		#[pallet::call_index(4)]
		#[pallet::weight(<T as Config>::WeightInfo::lock_self())]
		pub fn lock_self(
			origin: OriginFor<T>,
			amount: BalanceOf<T>,
			duration: BlockNumberFor<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(!amount.is_zero(), Error::<T>::ZeroAmount);
			ensure!(duration >= T::MinLockDuration::get(), Error::<T>::DurationTooShort);
			ensure!(duration <= T::MaxLockDuration::get(), Error::<T>::DurationTooLong);

			let now = frame_system::Pallet::<T>::block_number();
			let (total, until) = match SelfLocks::<T>::get(&who) {
				Some(l) if now < l.until =>
					(l.amount.saturating_add(amount), l.until.max(now.saturating_add(duration))),
				_ => (amount, now.saturating_add(duration)),
			};
			// `set_freeze` doesn't check the balance, so check it here, keeping `LockFeeReserve` free.
			ensure!(
				T::Currency::balance(&who) >= total.saturating_add(T::LockFeeReserve::get()),
				Error::<T>::InsufficientBalance
			);

			T::Currency::set_freeze(&FreezeReason::SelfLocked.into(), &who, total)?;
			SelfLocks::<T>::insert(&who, SelfLock { amount: total, until });

			Self::deposit_event(Event::SelfLocked { who, amount: total, until });
			Ok(())
		}

		/// Permissionless: release `who`'s self-lock once its expiry block has passed.
		#[pallet::call_index(5)]
		#[pallet::weight(<T as Config>::WeightInfo::unlock_expired())]
		pub fn unlock_expired(origin: OriginFor<T>, who: T::AccountId) -> DispatchResult {
			ensure_signed(origin)?;
			let lock = SelfLocks::<T>::get(&who).ok_or(Error::<T>::NotLocked)?;
			ensure!(frame_system::Pallet::<T>::block_number() >= lock.until, Error::<T>::NotYetExpired);

			T::Currency::thaw(&FreezeReason::SelfLocked.into(), &who)?;
			SelfLocks::<T>::remove(&who);

			Self::deposit_event(Event::SelfUnlocked { who, amount: lock.amount });
			Ok(())
		}
	}
}
