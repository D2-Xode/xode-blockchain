#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub use pallet::*;
pub mod weights;
pub use weights::WeightInfo;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[frame_support::pallet]
pub mod pallet {
	use alloc::{boxed::Box, vec::Vec};
	use frame_support::{
		pallet_prelude::*,
		traits::{fungibles::{Destroy, Refund}, Contains, IsSubType, SortedMembers},
		BoundedVec,
	};
	use frame_system::pallet_prelude::*;
	use pallet_asset_conversion::{Call as AssetConversionCall, PoolLocator, Pools as AssetConversionPools};
	use sp_runtime::traits::Dispatchable;

	#[derive(Clone, Copy, Debug, Decode, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen)]
	pub enum PoolStatus {
		Pending,
		Active,
	}

	#[derive(Clone, Debug, Decode, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen)]
	#[scale_info(skip_type_params(T))]
	pub struct PoolDetails<T: Config> {
		pub creator: T::AccountId,
		pub status: PoolStatus,
		pub approvals: BoundedVec<T::AccountId, ConstU32<2>>,
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_asset_conversion::Config {
		#[allow(deprecated)]
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		type RuntimeCall: Parameter
			+ Dispatchable<RuntimeOrigin = Self::RuntimeOrigin>
			+ frame_support::traits::IsSubType<AssetConversionCall<Self>>;

		type GovernanceMembers: SortedMembers<Self::AccountId>;

		type WeightInfo: crate::weights::WeightInfo;
	}

	#[pallet::storage]
	#[pallet::getter(fn pool_details)]
	pub type PoolDetailsById<T: Config> =
		StorageMap<_, Blake2_128Concat, T::PoolId, PoolDetails<T>, OptionQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		PoolCreatedPending {
			creator: T::AccountId,
			pool_id: T::PoolId,
		},
		PoolApproved {
			pool_id: T::PoolId,
			approver: T::AccountId,
			approvals: u32,
		},
		PoolActivated { pool_id: T::PoolId },
		PoolRejected { pool_id: T::PoolId, approver: T::AccountId },
	}

	#[pallet::error]
	pub enum Error<T> {
		InvalidAssetPair,
		PoolNotFound,
		PoolNotPending,
		NotGovernanceMember,
		AlreadyApproved,
		TooManyApprovals,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T>
	where
		T::PoolAssets: Destroy<T::AccountId>,
	{
		#[pallet::call_index(0)]
		#[pallet::weight(<<T as Config>::WeightInfo as crate::weights::WeightInfo>::create_pool())]
		pub fn create_pool(
			origin: OriginFor<T>,
			asset1: Box<T::AssetKind>,
			asset2: Box<T::AssetKind>,
		) -> DispatchResult {
			let creator = ensure_signed(origin)?;
			let pool_id = T::PoolLocator::pool_id(asset1.as_ref(), asset2.as_ref())
				.map_err(|_| Error::<T>::InvalidAssetPair)?;
			ensure!(!PoolDetailsById::<T>::contains_key(&pool_id), Error::<T>::PoolNotPending);

			pallet_asset_conversion::Pallet::<T>::create_pool(
				frame_system::RawOrigin::Signed(creator.clone()).into(),
				asset1.clone(),
				asset2.clone(),
			)?;

			PoolDetailsById::<T>::insert(
				pool_id.clone(),
				PoolDetails {
					creator: creator.clone(),
					status: PoolStatus::Pending,
					approvals: BoundedVec::default(),
				},
			);
			Self::deposit_event(Event::PoolCreatedPending { creator, pool_id });
			Ok(())
		}

		#[pallet::call_index(1)]
		#[pallet::weight(<<T as Config>::WeightInfo as crate::weights::WeightInfo>::approve_pool())]
		pub fn approve_pool(
			origin: OriginFor<T>,
			asset1: Box<T::AssetKind>,
			asset2: Box<T::AssetKind>,
		) -> DispatchResult {
			let approver = ensure_signed(origin)?;
			Self::ensure_governance_member(&approver)?;

			let pool_id = T::PoolLocator::pool_id(asset1.as_ref(), asset2.as_ref())
				.map_err(|_| Error::<T>::InvalidAssetPair)?;
			let mut approvals = 0u32;
			let mut activated = false;

			PoolDetailsById::<T>::try_mutate(pool_id.clone(), |maybe_details| -> DispatchResult {
				let details = maybe_details.as_mut().ok_or(Error::<T>::PoolNotFound)?;
				ensure!(details.status == PoolStatus::Pending, Error::<T>::PoolNotPending);
				ensure!(!details.approvals.contains(&approver), Error::<T>::AlreadyApproved);
				details
					.approvals
					.try_push(approver.clone())
					.map_err(|_| Error::<T>::TooManyApprovals)?;
				approvals = details.approvals.len() as u32;
				if approvals >= 2 {
					details.status = PoolStatus::Active;
					activated = true;
				}
				Ok(())
			})?;

			Self::deposit_event(Event::PoolApproved { pool_id: pool_id.clone(), approver, approvals });
			if activated {
				Self::deposit_event(Event::PoolActivated { pool_id });
			}
			Ok(())
		}

		#[pallet::call_index(2)]
		#[pallet::weight(<<T as Config>::WeightInfo as crate::weights::WeightInfo>::reject_pool())]
		pub fn reject_pool(
			origin: OriginFor<T>,
			asset1: Box<T::AssetKind>,
			asset2: Box<T::AssetKind>,
		) -> DispatchResult
		where
			T::PoolAssets: Destroy<T::AccountId>,
		{
			let approver = ensure_signed(origin)?;
			Self::ensure_governance_member(&approver)?;

			let pool_id = T::PoolLocator::pool_id(asset1.as_ref(), asset2.as_ref())
				.map_err(|_| Error::<T>::InvalidAssetPair)?;
			let details = PoolDetailsById::<T>::get(&pool_id).ok_or(Error::<T>::PoolNotFound)?;
			ensure!(details.status == PoolStatus::Pending, Error::<T>::PoolNotPending);
			let pool_info = AssetConversionPools::<T>::get(&pool_id).ok_or(Error::<T>::PoolNotFound)?;

			Self::cleanup_pending_pool(&pool_id, asset1.as_ref(), asset2.as_ref(), pool_info.lp_token)?;
			PoolDetailsById::<T>::remove(&pool_id);
			AssetConversionPools::<T>::remove(&pool_id);
			Self::deposit_event(Event::PoolRejected { pool_id, approver });
			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		fn cleanup_pending_pool(
			pool_id: &T::PoolId,
			asset1: &T::AssetKind,
			asset2: &T::AssetKind,
			lp_token: T::PoolAssetId,
		) -> DispatchResult
		where
			T::PoolAssets: Destroy<T::AccountId>,
		{
			let pool_account = T::PoolLocator::address(pool_id)
				.map_err(|_| Error::<T>::InvalidAssetPair)?;

			let _ = T::Assets::refund(asset1.clone(), pool_account.clone());
			let _ = T::Assets::refund(asset2.clone(), pool_account.clone());
			let _ = T::PoolAssets::refund(lp_token.clone(), pool_account.clone());

			<T::PoolAssets as Destroy<T::AccountId>>::start_destroy(
				lp_token.clone(),
				Some(pool_account),
			)?;
			while <T::PoolAssets as Destroy<T::AccountId>>::destroy_accounts(lp_token.clone(), u32::MAX)? > 0 {}
			while <T::PoolAssets as Destroy<T::AccountId>>::destroy_approvals(lp_token.clone(), u32::MAX)? > 0 {}
			<T::PoolAssets as Destroy<T::AccountId>>::finish_destroy(lp_token)?;
			Ok(())
		}

		fn ensure_governance_member(account: &T::AccountId) -> DispatchResult {
			ensure!(
				T::GovernanceMembers::sorted_members().binary_search(account).is_ok(),
				Error::<T>::NotGovernanceMember
			);
			Ok(())
		}

		fn ensure_pool_active(
			asset1: &T::AssetKind,
			asset2: &T::AssetKind,
		) -> DispatchResult {
			let pool_id = T::PoolLocator::pool_id(asset1, asset2)
				.map_err(|_| Error::<T>::InvalidAssetPair)?;
			if let Some(details) = PoolDetailsById::<T>::get(pool_id) {
				ensure!(details.status == PoolStatus::Active, Error::<T>::PoolNotPending);
			}

			Ok(())
		}

		pub fn ensure_path_active(path: &[T::AssetKind]) -> DispatchResult {
			if path.len() < 2 {
				return Ok(());
			}

			for assets in path.windows(2) {
				Self::ensure_pool_active(&assets[0], &assets[1])?;
			}

			Ok(())
		}

		pub fn contains(call: &<T as Config>::RuntimeCall) -> bool {
			match call.is_sub_type() {
				Some(AssetConversionCall::create_pool { .. }) => false,
				Some(AssetConversionCall::add_liquidity {
					asset1,
					asset2,
					..
				}) => Self::ensure_pool_active(asset1.as_ref(), asset2.as_ref()).is_ok(),
				Some(AssetConversionCall::remove_liquidity {
					asset1,
					asset2,
					..
				}) => Self::ensure_pool_active(asset1.as_ref(), asset2.as_ref()).is_ok(),
				Some(AssetConversionCall::swap_exact_tokens_for_tokens { path, .. }) => {
					let path: Vec<T::AssetKind> = path
						.iter()
						.map(|asset: &Box<T::AssetKind>| asset.as_ref().clone())
						.collect();
					Self::ensure_path_active(&path).is_ok()
				},
				Some(AssetConversionCall::swap_tokens_for_exact_tokens { path, .. }) => {
					let path: Vec<T::AssetKind> = path
						.iter()
						.map(|asset: &Box<T::AssetKind>| asset.as_ref().clone())
						.collect();
					Self::ensure_path_active(&path).is_ok()
				},
				_ => true,
			}
		}
	}

	impl<T: Config> Contains<<T as Config>::RuntimeCall> for Pallet<T> {
		fn contains(call: &<T as Config>::RuntimeCall) -> bool {
			Self::contains(call)
		}
	}
}