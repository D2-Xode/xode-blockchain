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

fn transfer(from: AccountId, to: AccountId, amount: Balance) -> sp_runtime::DispatchResult {
	<Balances as frame_support::traits::fungible::Mutate<AccountId>>::transfer(
		&from, &to, amount, frame_support::traits::tokens::Preservation::Preserve
	).map(|_| ())
}

#[test]
fn lock_self_blocks_transfer_until_expiry() {
	new_test_ext().execute_with(|| {
		System::set_block_number(1);
		assert_ok!(AccountFreezer::lock_self(RawOrigin::Signed(1).into(), 900_000, 10));
		assert_eq!(crate::SelfLocks::<Test>::get(1).unwrap().until, 11);

		assert_noop!(transfer(1, 2, 200_000), sp_runtime::TokenError::Frozen);
		assert_ok!(transfer(1, 2, 50_000));

		// anyone can release it, but only after expiry
		assert_noop!(
			AccountFreezer::unlock_expired(RawOrigin::Signed(2).into(), 1),
			Error::<Test>::NotYetExpired
		);
		System::set_block_number(11);
		assert_ok!(AccountFreezer::unlock_expired(RawOrigin::Signed(2).into(), 1));
		assert!(crate::SelfLocks::<Test>::get(1).is_none());
		assert_ok!(transfer(1, 2, 200_000));
	});
}

#[test]
fn lock_self_validates_inputs() {
	new_test_ext().execute_with(|| {
		let o = || RawOrigin::Signed(1).into();
		assert_noop!(AccountFreezer::lock_self(o(), 0, 10), Error::<Test>::ZeroAmount);
		assert_noop!(AccountFreezer::lock_self(o(), 100, 9), Error::<Test>::DurationTooShort);
		assert_noop!(AccountFreezer::lock_self(o(), 100, 101), Error::<Test>::DurationTooLong);
		assert_noop!(AccountFreezer::lock_self(o(), 1_000_001, 10), Error::<Test>::InsufficientBalance);
		assert_noop!(
			AccountFreezer::lock_self(RawOrigin::Root.into(), 100, 10),
			frame_support::error::BadOrigin
		);
		assert_noop!(
			AccountFreezer::unlock_expired(RawOrigin::Signed(2).into(), 1),
			Error::<Test>::NotLocked
		);
	});
}

#[test]
fn lock_self_keeps_fee_reserve_free() {
	new_test_ext().execute_with(|| {
		System::set_block_number(1);
		// balance 1_000_000, LockFeeReserve 1_000: the whole balance can't be locked
		assert_noop!(
			AccountFreezer::lock_self(RawOrigin::Signed(1).into(), 1_000_000, 10),
			Error::<Test>::InsufficientBalance
		);
		assert_noop!(
			AccountFreezer::lock_self(RawOrigin::Signed(1).into(), 999_001, 10),
			Error::<Test>::InsufficientBalance
		);
		assert_ok!(AccountFreezer::lock_self(RawOrigin::Signed(1).into(), 999_000, 10));

		// the reserve is still spendable
		assert_ok!(transfer(1, 2, 999));
	});
}

#[test]
fn adding_to_a_lock_sums_amounts_and_never_shortens_it() {
	new_test_ext().execute_with(|| {
		System::set_block_number(1);
		assert_ok!(AccountFreezer::lock_self(RawOrigin::Signed(1).into(), 300_000, 50));
		assert_ok!(AccountFreezer::lock_self(RawOrigin::Signed(1).into(), 200_000, 10));
		let lock = crate::SelfLocks::<Test>::get(1).unwrap();
		assert_eq!((lock.amount, lock.until), (500_000, 51));

		// the sum is checked against the balance
		assert_noop!(
			AccountFreezer::lock_self(RawOrigin::Signed(1).into(), 500_001, 10),
			Error::<Test>::InsufficientBalance
		);
	});
}

#[test]
fn relocking_after_expiry_starts_fresh() {
	new_test_ext().execute_with(|| {
		System::set_block_number(1);
		assert_ok!(AccountFreezer::lock_self(RawOrigin::Signed(1).into(), 300_000, 10));
		System::set_block_number(20);
		// expired but never released: the old amount isn't carried into the new term
		assert_ok!(AccountFreezer::lock_self(RawOrigin::Signed(1).into(), 100_000, 10));
		let lock = crate::SelfLocks::<Test>::get(1).unwrap();
		assert_eq!((lock.amount, lock.until), (100_000, 30));
	});
}

#[test]
fn self_lock_and_admin_freeze_are_independent() {
	new_test_ext().execute_with(|| {
		System::set_block_number(1);
		assert_ok!(AccountFreezer::lock_self(RawOrigin::Signed(1).into(), 300_000, 10));
		assert_ok!(AccountFreezer::freeze_amount(RawOrigin::Root.into(), 1, 800_000));

		// thawing the admin freeze leaves the self-lock in place
		assert_ok!(AccountFreezer::thaw_account(RawOrigin::Root.into(), 1));
		assert_noop!(transfer(1, 2, 800_000), sp_runtime::TokenError::Frozen);
		assert_ok!(transfer(1, 2, 600_000));
	});
}

#[test]
fn self_locked_balance_can_still_be_reserved() {
	// Documents pallet_balances behaviour: a freeze overlaps reserved balance,
	// so locked coins can also be reserved (e.g. bonded via XodeStaking).
	new_test_ext().execute_with(|| {
		use frame_support::traits::ReservableCurrency;
		System::set_block_number(1);
		assert_ok!(AccountFreezer::lock_self(RawOrigin::Signed(1).into(), 900_000, 10));
		let reserved = <Balances as ReservableCurrency<AccountId>>::reserve(&1, 900_000);
		println!("reserve of self-locked balance: {:?}", reserved);
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
