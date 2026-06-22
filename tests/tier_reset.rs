//! Tier reset during distribution: only qualified users' graduated accounts
//! reset, and reset failures do not roll back the distribution.
//!
//! The TierResetPort trait is the integration seam; royalflush does NOT
//! depend on flushline directly. A mock port exercises the contract.

use royalflush::{ResetOutcome, ResetPort, RoyalFlush};
use std::cell::RefCell;
use std::collections::HashSet;
use uuid::Uuid;

/// Records every reset call and can be configured to fail per-account.
struct MockReset {
    failures: RefCell<HashSet<Uuid>>,
    calls: RefCell<Vec<Uuid>>,
}

impl MockReset {
    fn new() -> Self {
        Self {
            failures: RefCell::new(HashSet::new()),
            calls: RefCell::new(Vec::new()),
        }
    }
    fn fail_for(&self, id: Uuid) {
        self.failures.borrow_mut().insert(id);
    }
    fn calls(&self) -> Vec<Uuid> {
        self.calls.borrow().clone()
    }
}

/// The crate's reset port returns a Result per account; we model success unless
/// the id is in our configured failures set.
impl ResetPort for MockReset {
    fn reset_to_king(&mut self, account_id: Uuid, _owner: String) -> Result<(), String> {
        self.calls.borrow_mut().push(account_id);
        if self.failures.borrow().contains(&account_id) {
            Err(format!("forced failure for {account_id}"))
        } else {
            Ok(())
        }
    }
}

fn qualify_with_two_graduated_accounts(rf: &mut RoyalFlush) -> (Uuid, Uuid, Uuid) {
    let user = Uuid::now_v7();
    let grad1 = Uuid::now_v7();
    let grad2 = Uuid::now_v7();
    let cycler = Uuid::now_v7();
    rf.register_user_account(user, grad1);
    rf.register_user_account(user, grad2);
    rf.register_user_account(user, cycler);
    // Dual qualification: graduated accounts + a cycled account.
    rf.record_graduation(grad1).unwrap();
    rf.record_graduation(grad2).unwrap();
    rf.record_matrix_cycle(cycler, Uuid::now_v7()).unwrap();
    (user, grad1, grad2)
}

#[test]
fn distribution_without_reset_port_skips_reset() {
    let mut rf = RoyalFlush::new();
    let (_user, _g1, _g2) = qualify_with_two_graduated_accounts(&mut rf);
    rf.add_points(1000);

    let (_result, outcome) = rf
        .distribute_weekly_with_reset(None::<&mut MockReset>)
        .unwrap();
    assert_eq!(outcome, ResetOutcome::Skipped);
}

#[test]
fn reset_only_targets_graduated_accounts_of_qualified_users() {
    let mut rf = RoyalFlush::new();
    let (_user, g1, g2) = qualify_with_two_graduated_accounts(&mut rf);
    // An unqualified user's graduated account must NOT be reset.
    let unq_user = Uuid::now_v7();
    let unq_grad = Uuid::now_v7();
    rf.register_user_account(unq_user, unq_grad);
    rf.record_graduation(unq_grad).unwrap();
    rf.add_points(1000);

    let mut port = MockReset::new();
    let (_result, outcome) = rf.distribute_weekly_with_reset(Some(&mut port)).unwrap();

    let mut reset = port.calls();
    reset.sort();
    let mut expected = vec![g1, g2];
    expected.sort();
    assert_eq!(
        reset, expected,
        "only qualified users' graduated accounts reset"
    );
    assert_eq!(outcome, ResetOutcome::All(2));
    assert!(!port.calls().contains(&unq_grad));
}

#[test]
fn distribution_succeeds_even_if_every_reset_fails() {
    let mut rf = RoyalFlush::new();
    let (_user, g1, g2) = qualify_with_two_graduated_accounts(&mut rf);
    rf.add_points(1000);

    let mut port = MockReset::new();
    port.fail_for(g1);
    port.fail_for(g2);
    let (_result, outcome) = rf.distribute_weekly_with_reset(Some(&mut port)).unwrap();

    assert_eq!(
        outcome,
        ResetOutcome::Partial {
            succeeded: 0,
            failed: 2
        }
    );
    // Pool still reset to 0; distribution was applied.
    assert_eq!(rf.total_pool_points(), 0);
}
