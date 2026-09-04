use crate::{mock::*, Error, Event, FrozenAccounts};
use frame_support::{assert_noop, assert_ok};
use frame_system::RawOrigin;

const ALICE: AccountId = 1;
const BOB: AccountId = 2;

#[test]
fn freeze_requires_freeze_origin() {
	new_test_ext(vec![(ALICE, 1_000)]).execute_with(|| {
		assert_noop!(
			XodeFreezer::freeze(RawOrigin::Signed(ALICE).into(), ALICE, 500),
			sp_runtime::DispatchError::BadOrigin,
		);
	});
}

#[test]
fn freeze_blocks_spending_beyond_the_frozen_amount() {
	new_test_ext(vec![(ALICE, 1_000)]).execute_with(|| {
		assert_ok!(XodeFreezer::freeze(RawOrigin::Root.into(), ALICE, 700));

		assert_eq!(FrozenAccounts::<Test>::get(ALICE), Some(700));
		System::assert_last_event(Event::BalanceFrozen { who: ALICE, amount: 700 }.into());

		// 1_000 free - 700 frozen = 300 spendable; asking for more must fail.
		assert_noop!(
			Balances::transfer_allow_death(RawOrigin::Signed(ALICE).into(), BOB, 400),
			sp_runtime::TokenError::Frozen,
		);

		// Spending within the still-usable balance succeeds.
		assert_ok!(Balances::transfer_allow_death(RawOrigin::Signed(ALICE).into(), BOB, 300));
	});
}

#[test]
fn freeze_replaces_rather_than_accumulates() {
	new_test_ext(vec![(ALICE, 1_000)]).execute_with(|| {
		assert_ok!(XodeFreezer::freeze(RawOrigin::Root.into(), ALICE, 700));
		assert_ok!(XodeFreezer::freeze(RawOrigin::Root.into(), ALICE, 200));

		assert_eq!(FrozenAccounts::<Test>::get(ALICE), Some(200));
		// Only 200 is now frozen, so 800 is spendable.
		assert_ok!(Balances::transfer_allow_death(RawOrigin::Signed(ALICE).into(), BOB, 800));
	});
}

#[test]
fn thaw_restores_full_spendability() {
	new_test_ext(vec![(ALICE, 1_000)]).execute_with(|| {
		assert_ok!(XodeFreezer::freeze(RawOrigin::Root.into(), ALICE, 700));
		assert_ok!(XodeFreezer::thaw(RawOrigin::Root.into(), ALICE));

		assert_eq!(FrozenAccounts::<Test>::get(ALICE), None);
		System::assert_last_event(Event::BalanceThawed { who: ALICE }.into());

		assert_ok!(Balances::transfer_allow_death(RawOrigin::Signed(ALICE).into(), BOB, 1_000));
	});
}

#[test]
fn thaw_requires_an_existing_freeze() {
	new_test_ext(vec![(ALICE, 1_000)]).execute_with(|| {
		assert_noop!(
			XodeFreezer::thaw(RawOrigin::Root.into(), ALICE),
			Error::<Test>::NotFrozen,
		);
	});
}

#[test]
fn thaw_requires_freeze_origin() {
	new_test_ext(vec![(ALICE, 1_000)]).execute_with(|| {
		assert_ok!(XodeFreezer::freeze(RawOrigin::Root.into(), ALICE, 700));

		assert_noop!(
			XodeFreezer::thaw(RawOrigin::Signed(ALICE).into(), ALICE),
			sp_runtime::DispatchError::BadOrigin,
		);
	});
}
