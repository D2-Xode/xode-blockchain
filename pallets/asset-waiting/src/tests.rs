use crate::{mock::*, pallet::{Error, PoolDetailsById, PoolStatus}};
use frame_support::{assert_noop, assert_ok};
use frame_support::traits::{fungible::NativeOrWithId, fungibles::Inspect};
use pallet_asset_conversion::Pools as AssetConversionPools;
use pallet_asset_conversion::PoolLocator;

macro_rules! bvec {
	($($x:expr),+ $(,)?) => (
		vec![$( Box::new($x), )*]
	)
}

#[test]
fn pool_is_created_in_pending_state() {
	new_test_ext().execute_with(|| {
		let user = 1;
		let native = NativeOrWithId::Native;
		let asset = NativeOrWithId::WithId(2);
		let pool_id = (native.clone(), asset.clone());

		create_tokens(user, vec![asset.clone()]);
		assert_ok!(AssetWaiting::create_pool(
			RuntimeOrigin::signed(user),
			Box::new(native.clone()),
			Box::new(asset.clone()),
		));

		let details = PoolDetailsById::<Test>::get(pool_id).expect("pending pool");
		assert_eq!(details.creator, user);
		assert_eq!(details.status, PoolStatus::Pending);
	});
}

#[test]
fn direct_asset_conversion_create_pool_call_is_rejected_by_filter() {
	new_test_ext().execute_with(|| {
		let native = NativeOrWithId::Native;
		let asset = NativeOrWithId::WithId(2);
		let call = create_pool_call(native, asset);
		assert!(!contains_call(&call));
	});
}

#[test]
fn add_liquidity_is_blocked_while_pool_is_pending() {
	new_test_ext().execute_with(|| {
		let user = 1;
		let native = NativeOrWithId::Native;
		let asset = NativeOrWithId::WithId(2);

		create_tokens(user, vec![asset.clone()]);
		assert_ok!(AssetWaiting::create_pool(
			RuntimeOrigin::signed(user),
			Box::new(native.clone()),
			Box::new(asset.clone()),
		));

		let call = add_liquidity_call(native, asset, user);
		assert!(!contains_call(&call));
	});
}

#[test]
fn pool_activates_after_two_governance_approvals() {
	new_test_ext().execute_with(|| {
		let user = 1;
		let native = NativeOrWithId::Native;
		let asset = NativeOrWithId::WithId(2);
		let pool_id = (native.clone(), asset.clone());

		create_tokens(user, vec![asset.clone()]);
		assert_ok!(AssetWaiting::create_pool(
			RuntimeOrigin::signed(user),
			Box::new(native.clone()),
			Box::new(asset.clone()),
		));
		assert_ok!(AssetWaiting::approve_pool(
			RuntimeOrigin::signed(10),
			Box::new(native.clone()),
			Box::new(asset.clone()),
		));
		assert_ok!(AssetWaiting::approve_pool(
			RuntimeOrigin::signed(11),
			Box::new(native.clone()),
			Box::new(asset.clone()),
		));

		let details = PoolDetailsById::<Test>::get(pool_id).expect("active pool");
		assert_eq!(details.status, PoolStatus::Active);

		let add_call = add_liquidity_call(native.clone(), asset.clone(), user);
		assert!(contains_call(&add_call));

		let swap = swap_call(bvec![asset, native], user);
		assert!(contains_call(&swap));
	});
}

#[test]
fn duplicate_approval_is_rejected() {
	new_test_ext().execute_with(|| {
		let user = 1;
		let native = NativeOrWithId::Native;
		let asset = NativeOrWithId::WithId(2);

		create_tokens(user, vec![asset.clone()]);
		assert_ok!(AssetWaiting::create_pool(
			RuntimeOrigin::signed(user),
			Box::new(native.clone()),
			Box::new(asset.clone()),
		));
		assert_ok!(AssetWaiting::approve_pool(
			RuntimeOrigin::signed(10),
			Box::new(native.clone()),
			Box::new(asset.clone()),
		));

		assert_noop!(
			AssetWaiting::approve_pool(
				RuntimeOrigin::signed(10),
				Box::new(native),
				Box::new(asset),
			),
			Error::<Test>::AlreadyApproved
		);
	});
}


#[test]
fn non_governance_approval_is_rejected() {
	new_test_ext().execute_with(|| {
		let user = 1;
		let native = NativeOrWithId::Native;
		let asset = NativeOrWithId::WithId(2);

		create_tokens(user, vec![asset.clone()]);
		assert_ok!(AssetWaiting::create_pool(
			RuntimeOrigin::signed(user),
			Box::new(native.clone()),
			Box::new(asset.clone()),
		));

		assert_noop!(
			AssetWaiting::approve_pool(
				RuntimeOrigin::signed(user),
				Box::new(native),
				Box::new(asset),
			),
			Error::<Test>::NotGovernanceMember
		);
	});
}

#[test]
fn rejection_by_one_governance_member_deletes_pending_pool() {
	new_test_ext().execute_with(|| {
		let user = 1;
		let native = NativeOrWithId::Native;
		let asset = NativeOrWithId::WithId(2);
		let pool_id = (native.clone(), asset.clone());
		let pool_account = <Test as pallet_asset_conversion::Config>::PoolLocator::address(&pool_id)
			.expect("pool account");

		create_tokens(user, vec![asset.clone()]);
		assert_ok!(AssetWaiting::create_pool(
			RuntimeOrigin::signed(user),
			Box::new(native.clone()),
			Box::new(asset.clone()),
		));

		let lp_token = AssetConversionPools::<Test>::get(pool_id.clone())
			.expect("pool info")
			.lp_token;

		assert_ok!(AssetWaiting::reject_pool(
			RuntimeOrigin::signed(10),
			Box::new(native.clone()),
			Box::new(asset.clone()),
		));

		assert!(PoolDetailsById::<Test>::get(pool_id.clone()).is_none());
		assert!(AssetConversionPools::<Test>::get(pool_id).is_none());
		assert!(!PoolAssets::asset_exists(lp_token));
		assert!(!System::account_exists(&pool_account));

		let call = add_liquidity_call(native, asset, user);
		assert!(contains_call(&call));
	});
}