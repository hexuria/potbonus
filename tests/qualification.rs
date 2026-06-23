//! User-level qualification tracking: dual qualification, user-level cycle
//! aggregation, top-performer ranking. Ported from rfn's
//! `UserQualificationTracker` invariants.

use potbonus::{PotBonus, UserQualification};
use uuid::Uuid;

#[test]
fn user_starts_unqualified() {
    let rf = PotBonus::new();
    let user = Uuid::now_v7();
    let _ = user;
    // No accounts registered -> not qualified.
    assert!(rf.qualified_users().is_empty());
}

#[test]
fn graduation_alone_does_not_qualify() {
    let mut rf = PotBonus::new();
    let (user, account) = register_one_account(&mut rf);

    rf.record_graduation(account).unwrap();
    assert!(
        !rf.is_user_qualified(&user),
        "graduation alone is not enough"
    );
}

#[test]
fn matrix_cycle_alone_does_not_qualify() {
    let mut rf = PotBonus::new();
    let (user, account) = register_one_account(&mut rf);
    let matrix = Uuid::now_v7();

    rf.record_matrix_cycle(account, matrix).unwrap();
    assert!(!rf.is_user_qualified(&user), "cycle alone is not enough");
}

#[test]
fn dual_qualification_graduation_plus_cycle_qualifies() {
    let mut rf = PotBonus::new();
    let (user, account) = register_one_account(&mut rf);
    let matrix = Uuid::now_v7();

    rf.record_graduation(account).unwrap();
    assert!(!rf.is_user_qualified(&user));
    rf.record_matrix_cycle(account, matrix).unwrap();
    assert!(rf.is_user_qualified(&user));
}

#[test]
fn dual_qualification_can_span_different_accounts_of_same_user() {
    let mut rf = PotBonus::new();
    let user = Uuid::now_v7();
    let account_a = Uuid::now_v7();
    let account_b = Uuid::now_v7();
    rf.register_user_account(user, account_a);
    rf.register_user_account(user, account_b);

    rf.record_graduation(account_a).unwrap();
    rf.record_matrix_cycle(account_b, Uuid::now_v7()).unwrap();
    assert!(
        rf.is_user_qualified(&user),
        "graduation + cycle on different accounts qualifies the user"
    );
}

#[test]
fn cycle_count_aggregates_across_all_user_accounts() {
    let mut rf = PotBonus::new();
    let user = Uuid::now_v7();
    let a = Uuid::now_v7();
    let b = Uuid::now_v7();
    let c = Uuid::now_v7();
    for acct in [a, b, c] {
        rf.register_user_account(user, acct);
    }

    rf.record_matrix_cycle(a, Uuid::now_v7()).unwrap();
    rf.record_matrix_cycle(a, Uuid::now_v7()).unwrap();
    rf.record_matrix_cycle(b, Uuid::now_v7()).unwrap();
    rf.record_matrix_cycle(c, Uuid::now_v7()).unwrap();

    assert_eq!(rf.user_cycle_count(&user), 4, "4 cycles across 3 accounts");
}

#[test]
fn top_performers_rank_by_cycle_count_then_graduation_count() {
    let mut rf = PotBonus::new();
    let make_user = |rf: &mut PotBonus, cycles: u32, grads: u32| -> Uuid {
        let user = Uuid::now_v7();
        for i in 0..cycles.max(grads) {
            let acct = Uuid::now_v7();
            rf.register_user_account(user, acct);
            if i < cycles {
                rf.record_matrix_cycle(acct, Uuid::now_v7()).unwrap();
            }
            if i < grads {
                rf.record_graduation(acct).unwrap();
            }
        }
        user
    };
    // high performer: 5 cycles
    let _u_high = make_user(&mut rf, 5, 1);
    // tied on cycles, differ on graduations
    let _u_more_grads = make_user(&mut rf, 3, 2);
    let _u_fewer_grads = make_user(&mut rf, 3, 1);

    let top = rf.top_performers(10);
    assert_eq!(top.len(), 3);
    assert_eq!(top[0].cycle_count, 5); // 5 cycles ranks first
    assert_eq!(top[1].graduation_count, 2, "tie broken by more graduations");
    assert_eq!(top[2].graduation_count, 1);
    let _: Option<UserQualification> = None; // type is exported
}

#[test]
fn record_events_for_unregistered_account_errors() {
    let mut rf = PotBonus::new();
    let unknown = Uuid::now_v7();
    assert!(rf.record_graduation(unknown).is_err());
    assert!(rf.record_matrix_cycle(unknown, Uuid::now_v7()).is_err());
}

fn register_one_account(rf: &mut PotBonus) -> (Uuid, Uuid) {
    let user = Uuid::now_v7();
    let account = Uuid::now_v7();
    rf.register_user_account(user, account);
    (user, account)
}
