//! Edge cases & rounding boundaries. Ported from rfn's
//! performance_and_stress_tests and comprehensive_business_rules.

use potbonus::PotBonus;
use uuid::Uuid;

fn qualify(rf: &mut PotBonus) -> Uuid {
    let user = Uuid::now_v7();
    let acct = Uuid::now_v7();
    rf.register_user_account(user, acct);
    rf.record_graduation(acct).unwrap();
    rf.record_matrix_cycle(acct, Uuid::now_v7()).unwrap();
    user
}

#[test]
fn one_point_pool_yields_zero_distribution_but_entries() {
    // (1 as f32 * 0.75) as u32 == 0; (1 as f32 * 0.25) as u32 == 0.
    let mut rf = PotBonus::new();
    let user = qualify(&mut rf);
    rf.add_points(1);
    let result = rf.distribute_weekly().unwrap();
    assert_eq!(result.profit_sharing_for(user), Some(0));
    assert_eq!(result.top_cycler_distributions[0].1, 0);
}

#[test]
fn two_point_pool_rounds_to_one_zero() {
    // 2 * 0.75 = 1.5 -> 1; 2 * 0.25 = 0.5 -> 0.
    let mut rf = PotBonus::new();
    let user = qualify(&mut rf);
    rf.add_points(2);
    let result = rf.distribute_weekly().unwrap();
    assert_eq!(result.profit_sharing_for(user), Some(1));
    assert_eq!(result.top_cycler_distributions[0].1, 0);
}

#[test]
fn distribution_is_user_level_not_account_level() {
    // A user with multiple accounts gets ONE profit-sharing entry, not N.
    let mut rf = PotBonus::new();
    let user = qualify(&mut rf);
    // Add more accounts to the same user — still one profit entry.
    for _ in 0..5 {
        let acct = Uuid::now_v7();
        rf.register_user_account(user, acct);
        rf.record_matrix_cycle(acct, Uuid::now_v7()).unwrap();
    }
    rf.add_points(1000);
    let result = rf.distribute_weekly().unwrap();
    assert_eq!(result.profit_sharing_distributions.len(), 1);
    assert!(result.profit_sharing_for(user).is_some());
}

#[test]
fn whale_dominates_top_performer_share_but_profit_stays_equal() {
    let mut rf = PotBonus::new();
    let normal1 = qualify(&mut rf);
    let normal2 = qualify(&mut rf);
    let whale = qualify(&mut rf);
    // Pump the whale.
    for _ in 0..100 {
        let acct = Uuid::now_v7();
        rf.register_user_account(whale, acct);
        rf.record_matrix_cycle(acct, Uuid::now_v7()).unwrap();
    }
    rf.add_points(1000);
    let result = rf.distribute_weekly().unwrap();

    // Profit sharing equal across all three.
    assert_eq!(result.profit_sharing_for(normal1), Some(250));
    assert_eq!(result.profit_sharing_for(normal2), Some(250));
    assert_eq!(result.profit_sharing_for(whale), Some(250));
    // Whale takes the top 40% of the 25% bucket.
    let top = result
        .top_cycler_for(whale)
        .expect("whale is the top performer");
    assert_eq!(top, 100);
}
