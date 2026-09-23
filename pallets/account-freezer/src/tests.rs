use crate::mock::*;
use crate::Error;
use frame_support::{assert_noop, assert_ok};
use frame_system::RawOrigin;

#[test]
fn freeze_blocks_transfer_and_thaw_lifts_it() {
	new_test_ext().execute_with(|| {
		assert_ok!(AccountFreezer::freeze_account(RawOrigin::Root.into(), 1));

		// frozen: transfer should fail
		assert_noop!(
			<Balances as frame_support::traits::fungible::Mutate<AccountId>>::transfer(
				&1, &2, 100, frame_support::traits::tokens::Preservation::Preserve
			),
			sp_runtime::TokenError::Frozen
		);

		assert_ok!(AccountFreezer::thaw_account(RawOrigin::Root.into(), 1));

		assert_ok!(<Balances as frame_support::traits::fungible::Mutate<AccountId>>::transfer(
			&1, &2, 100, frame_support::traits::tokens::Preservation::Preserve
		));
	});
}

#[test]
fn freeze_amount_blocks_only_that_amount() {
	new_test_ext().execute_with(|| {
		assert_ok!(AccountFreezer::freeze_amount(RawOrigin::Root.into(), 1, 900_000));

		// the frozen amount can't be moved below the freeze threshold
		assert_noop!(
			<Balances as frame_support::traits::fungible::Mutate<AccountId>>::transfer(
				&1, &2, 200_000, frame_support::traits::tokens::Preservation::Preserve
			),
			sp_runtime::TokenError::Frozen
		);

		// the remaining unfrozen balance is still transferable
		assert_ok!(<Balances as frame_support::traits::fungible::Mutate<AccountId>>::transfer(
			&1, &2, 50_000, frame_support::traits::tokens::Preservation::Preserve
		));

		assert_ok!(AccountFreezer::thaw_account(RawOrigin::Root.into(), 1));
	});
}

#[test]
fn cannot_double_freeze() {
	new_test_ext().execute_with(|| {
		assert_ok!(AccountFreezer::freeze_account(RawOrigin::Root.into(), 1));
		assert_noop!(
			AccountFreezer::freeze_account(RawOrigin::Root.into(), 1),
			Error::<Test>::AlreadyFrozen
		);
		assert_noop!(
			AccountFreezer::freeze_amount(RawOrigin::Root.into(), 1, 100),
			Error::<Test>::AlreadyFrozen
		);
	});
}

#[test]
fn non_root_cannot_freeze() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			AccountFreezer::freeze_account(RawOrigin::Signed(2).into(), 1),
			frame_support::error::BadOrigin
		);
		assert_noop!(
			AccountFreezer::freeze_amount(RawOrigin::Signed(2).into(), 1, 100),
			frame_support::error::BadOrigin
		);
	});
}

#[test]
fn force_origin_can_thaw() {
	new_test_ext().execute_with(|| {
		assert_ok!(AccountFreezer::freeze_account(RawOrigin::Root.into(), 1));
		assert_ok!(AccountFreezer::thaw_account(RawOrigin::Root.into(), 1));
		assert_noop!(
			AccountFreezer::thaw_account(RawOrigin::Root.into(), 1),
			Error::<Test>::NotFrozen
		);
	});
}

#[test]
fn thaw_amount_releases_only_part_of_the_freeze() {
	new_test_ext().execute_with(|| {
		assert_ok!(AccountFreezer::freeze_amount(RawOrigin::Root.into(), 1, 900_000));
		assert_ok!(AccountFreezer::thaw_amount(RawOrigin::Root.into(), 1, 400_000));

		// still frozen: only 500_000 remains free, so this exceeds it
		assert_noop!(
			<Balances as frame_support::traits::fungible::Mutate<AccountId>>::transfer(
				&1, &2, 600_000, frame_support::traits::tokens::Preservation::Preserve
			),
			sp_runtime::TokenError::Frozen
		);

		// within the newly freed headroom
		assert_ok!(<Balances as frame_support::traits::fungible::Mutate<AccountId>>::transfer(
			&1, &2, 450_000, frame_support::traits::tokens::Preservation::Preserve
		));
	});
}

#[test]
fn thaw_amount_covering_the_whole_freeze_fully_thaws() {
	new_test_ext().execute_with(|| {
		assert_ok!(AccountFreezer::freeze_amount(RawOrigin::Root.into(), 1, 900_000));
		assert_ok!(AccountFreezer::thaw_amount(RawOrigin::Root.into(), 1, 900_000));

		// fully thawed: another freeze is allowed again
		assert_ok!(AccountFreezer::freeze_account(RawOrigin::Root.into(), 1));
	});
}

#[test]
fn thaw_amount_more_than_frozen_fails() {
	new_test_ext().execute_with(|| {
		assert_ok!(AccountFreezer::freeze_amount(RawOrigin::Root.into(), 1, 100));
		assert_noop!(
			AccountFreezer::thaw_amount(RawOrigin::Root.into(), 1, 200),
			Error::<Test>::AmountExceedsFrozen
		);
	});
}

#[test]
fn thaw_amount_on_unfrozen_account_fails() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			AccountFreezer::thaw_amount(RawOrigin::Root.into(), 1, 100),
			Error::<Test>::NotFrozen
		);
	});
}
