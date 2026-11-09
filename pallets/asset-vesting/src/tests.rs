use crate::{mock::*, Error, Event, FreezeReason, VestingSchedule, VestingSchedules};
use frame_support::{
	assert_noop, assert_ok,
	traits::{
		fungibles::{Inspect, InspectFreeze, Mutate},
		tokens::{Fortitude, Preservation},
	},
};
use frame_system::RawOrigin;
use sp_runtime::{ArithmeticError, DispatchError, DispatchResult, TokenError};

type Schedule = VestingSchedule<u64, Balance>;

/// 200 in total: 100 unlocks at block 20 and 100 at block 30.
fn schedule() -> Schedule {
	VestingSchedule { start: 10, period: 10, period_count: 2, per_period: 100 }
}

fn vest(from: AccountId, asset: AssetId, to: AccountId, schedule: Schedule) -> DispatchResult {
	AssetVesting::vested_transfer(RuntimeOrigin::signed(from), asset, to, schedule)
}

fn frozen(who: AccountId, asset: AssetId) -> Balance {
	<AssetsFreezer as InspectFreeze<AccountId>>::balance_frozen(asset, &FreezeReason::Vesting.into(), &who)
}

fn balance(who: AccountId, asset: AssetId) -> Balance {
	<Assets as Inspect<AccountId>>::balance(asset, &who)
}

/// What `who` can move right now.
fn transferable(who: AccountId, asset: AssetId) -> Balance {
	<Assets as Inspect<AccountId>>::reducible_balance(asset, &who, Preservation::Expendable, Fortitude::Polite)
}

fn transfer(asset: AssetId, from: AccountId, to: AccountId, amount: Balance) -> DispatchResult {
	<Assets as Mutate<AccountId>>::transfer(asset, &from, &to, amount, Preservation::Expendable).map(|_| ())
}

#[test]
fn admin_can_vest_any_asset() {
	new_test_ext().execute_with(|| {
		assert_ok!(vest(ADMIN, ASSET, ALICE, schedule()));

		assert_eq!(balance(ADMIN, ASSET), INITIAL - 200);
		assert_eq!(balance(ALICE, ASSET), 200);
		assert_eq!(frozen(ALICE, ASSET), 200);
		assert_eq!(transferable(ALICE, ASSET), 0);
		assert_eq!(VestingSchedules::<Test>::get(ALICE, ASSET).into_inner(), vec![schedule()]);
		System::assert_last_event(
			Event::VestingScheduleAdded { asset_id: ASSET, from: ADMIN, to: ALICE, vesting_schedule: schedule() }
				.into(),
		);

		// the admin doesn't own OTHER_ASSET, but can vest it once it holds some
		assert_ok!(transfer(OTHER_ASSET, OTHER_OWNER, ADMIN, 1_000));
		assert_ok!(vest(ADMIN, OTHER_ASSET, ALICE, schedule()));
		assert_eq!(frozen(ALICE, OTHER_ASSET), 200);
	});
}

#[test]
fn owner_can_vest_only_its_own_asset() {
	new_test_ext().execute_with(|| {
		assert_ok!(vest(OWNER, ASSET, ALICE, schedule()));
		assert_eq!(frozen(ALICE, ASSET), 200);

		// holding OTHER_ASSET isn't enough; OWNER doesn't own it
		assert_ok!(transfer(OTHER_ASSET, OTHER_OWNER, OWNER, 1_000));
		assert_noop!(vest(OWNER, OTHER_ASSET, ALICE, schedule()), Error::<Test>::NotAdminOrAssetOwner);
	});
}

#[test]
fn others_cannot_vest() {
	new_test_ext().execute_with(|| {
		// ALICE holds ASSET but is neither an admin nor its owner
		assert_ok!(transfer(ASSET, OWNER, ALICE, 1_000));
		assert_noop!(vest(ALICE, ASSET, BOB, schedule()), Error::<Test>::NotAdminOrAssetOwner);
		// an unknown asset has no owner
		assert_noop!(vest(ALICE, 99, BOB, schedule()), Error::<Test>::NotAdminOrAssetOwner);
		assert_noop!(
			AssetVesting::vested_transfer(RawOrigin::Root.into(), ASSET, BOB, schedule()),
			DispatchError::BadOrigin
		);
	});
}

#[test]
fn claim_unfreezes_as_periods_pass() {
	new_test_ext().execute_with(|| {
		assert_ok!(vest(OWNER, ASSET, ALICE, schedule()));

		System::set_block_number(19);
		assert_ok!(AssetVesting::claim(RuntimeOrigin::signed(ALICE), ASSET));
		assert_eq!(frozen(ALICE, ASSET), 200);
		System::assert_last_event(Event::Claimed { asset_id: ASSET, who: ALICE, unlocked: 0, remaining: 200 }.into());

		System::set_block_number(20);
		assert_ok!(AssetVesting::claim(RuntimeOrigin::signed(ALICE), ASSET));
		assert_eq!(frozen(ALICE, ASSET), 100);
		assert_eq!(transferable(ALICE, ASSET), 100);
		assert_noop!(transfer(ASSET, ALICE, BOB, 101), TokenError::Frozen);
		System::assert_last_event(Event::Claimed { asset_id: ASSET, who: ALICE, unlocked: 100, remaining: 100 }.into());

		System::set_block_number(30);
		assert_ok!(AssetVesting::claim(RuntimeOrigin::signed(ALICE), ASSET));
		assert_eq!(frozen(ALICE, ASSET), 0);
		assert!(!VestingSchedules::<Test>::contains_key(ALICE, ASSET));
		System::assert_last_event(Event::Claimed { asset_id: ASSET, who: ALICE, unlocked: 100, remaining: 0 }.into());
		assert_ok!(transfer(ASSET, ALICE, BOB, 200));

		// nothing left to claim
		assert_noop!(AssetVesting::claim(RuntimeOrigin::signed(ALICE), ASSET), Error::<Test>::NotVesting);
	});
}

#[test]
fn anyone_can_claim_for_another_account() {
	new_test_ext().execute_with(|| {
		assert_ok!(vest(OWNER, ASSET, ALICE, schedule()));
		System::set_block_number(30);

		assert_ok!(AssetVesting::claim_for(RuntimeOrigin::signed(BOB), ASSET, ALICE));
		assert_eq!(frozen(ALICE, ASSET), 0);
		assert_eq!(transferable(ALICE, ASSET), 200);
	});
}

#[test]
fn claim_without_schedules_fails() {
	new_test_ext().execute_with(|| {
		assert_noop!(AssetVesting::claim(RuntimeOrigin::signed(ALICE), ASSET), Error::<Test>::NotVesting);
		assert_noop!(
			AssetVesting::claim_for(RuntimeOrigin::signed(BOB), ASSET, ALICE),
			Error::<Test>::NotVesting
		);
	});
}

#[test]
fn rejects_invalid_schedules() {
	new_test_ext().execute_with(|| {
		let vest = |s: Schedule| vest(OWNER, ASSET, ALICE, s);
		assert_noop!(vest(Schedule { period: 0, ..schedule() }), Error::<Test>::ZeroVestingPeriod);
		assert_noop!(vest(Schedule { period_count: 0, ..schedule() }), Error::<Test>::ZeroVestingPeriodCount);
		// 2 * 4 = 8, below MIN_BALANCE
		assert_noop!(vest(Schedule { per_period: 4, ..schedule() }), Error::<Test>::AmountLow);
		assert_noop!(vest(Schedule { per_period: 0, ..schedule() }), Error::<Test>::AmountLow);
		assert_noop!(vest(Schedule { per_period: Balance::MAX, ..schedule() }), ArithmeticError::Overflow);
		assert_noop!(vest(Schedule { start: u64::MAX, ..schedule() }), ArithmeticError::Overflow);
		// more than OWNER holds
		assert_noop!(vest(Schedule { per_period: INITIAL, ..schedule() }), TokenError::FundsUnavailable);
	});
}

#[test]
fn limits_schedules_per_account_and_asset() {
	new_test_ext().execute_with(|| {
		for _ in 0..3 {
			assert_ok!(vest(OWNER, ASSET, ALICE, schedule()));
		}
		assert_noop!(vest(OWNER, ASSET, ALICE, schedule()), Error::<Test>::MaxVestingSchedulesExceeded);

		// the limit is per asset
		assert_ok!(vest(OTHER_OWNER, OTHER_ASSET, ALICE, schedule()));

		// finished schedules free their slots
		System::set_block_number(30);
		assert_ok!(vest(OWNER, ASSET, ALICE, Schedule { start: 30, ..schedule() }));
		assert_eq!(VestingSchedules::<Test>::get(ALICE, ASSET).len(), 1);
		assert_eq!(frozen(ALICE, ASSET), 200);
	});
}

#[test]
fn owner_can_vest_to_itself() {
	new_test_ext().execute_with(|| {
		assert_ok!(vest(OWNER, ASSET, OWNER, schedule()));
		assert_eq!(balance(OWNER, ASSET), INITIAL);
		assert_eq!(frozen(OWNER, ASSET), 200);
		assert_eq!(transferable(OWNER, ASSET), INITIAL - 200);

		// already-frozen tokens can't be vested again
		assert_noop!(
			vest(OWNER, ASSET, OWNER, Schedule { per_period: INITIAL / 2, ..schedule() }),
			TokenError::Frozen
		);
	});
}

#[test]
fn each_asset_vests_independently() {
	new_test_ext().execute_with(|| {
		assert_ok!(vest(OWNER, ASSET, ALICE, schedule()));
		assert_ok!(vest(OTHER_OWNER, OTHER_ASSET, ALICE, Schedule { start: 100, ..schedule() }));

		System::set_block_number(30);
		assert_ok!(AssetVesting::claim(RuntimeOrigin::signed(ALICE), ASSET));
		assert_eq!(frozen(ALICE, ASSET), 0);
		assert_eq!(frozen(ALICE, OTHER_ASSET), 200);

		assert_ok!(AssetVesting::claim(RuntimeOrigin::signed(ALICE), OTHER_ASSET));
		assert_eq!(frozen(ALICE, OTHER_ASSET), 200);
	});
}

#[test]
fn force_origin_can_replace_schedules() {
	new_test_ext().execute_with(|| {
		assert_ok!(vest(OWNER, ASSET, ALICE, schedule()));
		assert_noop!(
			AssetVesting::update_vesting_schedules(RuntimeOrigin::signed(ADMIN), ASSET, ALICE, vec![]),
			DispatchError::BadOrigin
		);

		let replacement = Schedule { per_period: 50, period_count: 3, ..schedule() };
		assert_ok!(AssetVesting::update_vesting_schedules(
			RawOrigin::Root.into(),
			ASSET,
			ALICE,
			vec![replacement.clone()]
		));
		assert_eq!(frozen(ALICE, ASSET), 150);
		assert_eq!(VestingSchedules::<Test>::get(ALICE, ASSET).into_inner(), vec![replacement]);
		System::assert_last_event(Event::VestingSchedulesUpdated { asset_id: ASSET, who: ALICE }.into());

		// can't freeze more than ALICE holds (300 > 200)
		assert_noop!(
			AssetVesting::update_vesting_schedules(
				RawOrigin::Root.into(),
				ASSET,
				ALICE,
				vec![Schedule { per_period: 150, ..schedule() }]
			),
			Error::<Test>::InsufficientBalanceToLock
		);

		// an empty list unfreezes everything
		assert_ok!(AssetVesting::update_vesting_schedules(RawOrigin::Root.into(), ASSET, ALICE, vec![]));
		assert_eq!(frozen(ALICE, ASSET), 0);
		assert!(!VestingSchedules::<Test>::contains_key(ALICE, ASSET));
		assert_eq!(transferable(ALICE, ASSET), 200);
	});
}

#[test]
fn non_sufficient_asset_needs_an_existing_recipient() {
	new_test_ext().execute_with(|| {
		assert_noop!(vest(OWNER, ASSET, NO_XON, schedule()), TokenError::CannotCreate);
		// a sufficient asset can create the recipient's account
		assert_ok!(vest(OTHER_OWNER, OTHER_ASSET, NO_XON, schedule()));
		assert_eq!(frozen(NO_XON, OTHER_ASSET), 200);
	});
}

#[test]
fn asset_roles_cannot_reach_vesting_tokens() {
	new_test_ext().execute_with(|| {
		assert_ok!(vest(OWNER, ASSET, ALICE, schedule()));

		// the owner can't start destroying the asset while any balance is frozen
		assert_noop!(
			Assets::start_destroy(RuntimeOrigin::signed(OWNER), ASSET),
			pallet_assets::Error::<Test, frame_support::instances::Instance1>::ContainsFreezes
		);
		// an admin burn only reaches unfrozen balance, so nothing is burned
		assert_ok!(Assets::burn(RuntimeOrigin::signed(OWNER), ASSET, ALICE, 200));
		assert_eq!(balance(ALICE, ASSET), 200);
	});
}
