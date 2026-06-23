//! The 75-25 weekly distribution. Ported from rfn's distribution_invariants
//! and comprehensive_business_rules tests.
//!
//! Model:
//!   - 75% of the pool -> equal split among qualified users (integer division,
//!     remainder forfeited).
//!   - 25% of the pool -> top performers using the policy's percentage table
//!     (default [40, 30, 20, 10] of the 25% bucket), top-4 only; unused
//!     remainder is forfeited.
//!   - Pool resets to 0 after distribution (no rollover).
//!   - Tracker fully resets after distribution.

use potbonus::{DistributionPolicy, PotBonus};
use uuid::Uuid;

fn qualify_user(rf: &mut PotBonus) -> Uuid {
    let user = Uuid::now_v7();
    let acct = Uuid::now_v7();
    rf.register_user_account(user, acct);
    rf.record_graduation(acct).unwrap();
    rf.record_matrix_cycle(acct, Uuid::now_v7()).unwrap();
    user
}

#[test]
fn zero_pool_distribution_errors() {
    let mut rf = PotBonus::new();
    let _ = qualify_user(&mut rf);
    let result = rf.distribute_weekly();
    assert!(result.is_err());
}

#[test]
fn no_qualified_users_returns_empty_result_and_preserves_pool() {
    let mut rf = PotBonus::new();
    rf.add_points(1000);
    let result = rf.distribute_weekly().unwrap();
    assert_eq!(result.total_distributed, 0);
    assert_eq!(
        rf.total_pool_points(),
        1000,
        "pool preserved when nobody qualifies"
    );
}

#[test]
fn single_qualified_user_gets_exact_75_percent_profit_share() {
    let mut rf = PotBonus::new();
    let user = qualify_user(&mut rf);
    rf.add_points(1000);

    let result = rf.distribute_weekly().unwrap();
    let profit = result
        .profit_sharing_for(user)
        .expect("user got profit share");
    assert_eq!(profit, 750); // 1000 * 0.75
}

#[test]
fn profit_sharing_splits_equally_across_qualified_users() {
    let mut rf = PotBonus::new();
    let u1 = qualify_user(&mut rf);
    let u2 = qualify_user(&mut rf);
    let u3 = qualify_user(&mut rf);
    rf.add_points(1000);

    let result = rf.distribute_weekly().unwrap();
    // 750 / 3 = 250 each (no remainder).
    assert_eq!(result.profit_sharing_for(u1), Some(250));
    assert_eq!(result.profit_sharing_for(u2), Some(250));
    assert_eq!(result.profit_sharing_for(u3), Some(250));
}

#[test]
fn top_performer_split_uses_40_30_20_10_of_the_25_percent_bucket() {
    let mut rf = PotBonus::new();
    // 4 qualified users with strictly increasing cycle counts.
    let users: Vec<_> = (0..4).map(|_| qualify_user(&mut rf)).collect();
    // Pump up cycles so the ordering is deterministic.
    for (i, u) in users.iter().enumerate() {
        let acct = Uuid::now_v7();
        rf.register_user_account(*u, acct);
        for _ in 0..(i as u32 + 1) {
            rf.record_matrix_cycle(acct, Uuid::now_v7()).unwrap();
        }
    }
    rf.add_points(1000);

    let result = rf.distribute_weekly().unwrap();
    let total_top: u32 = result.top_cycler_distributions.iter().map(|(_, p)| p).sum();
    assert_eq!(total_top, 250, "25% of 1000");

    // The top-4 percentages of the 25% bucket (1000 -> 250):
    //   40% -> 100, 30% -> 75, 20% -> 50, 10% -> 25
    let mut sorted: Vec<u32> = result
        .top_cycler_distributions
        .iter()
        .map(|(_, p)| *p)
        .collect();
    sorted.sort_by(|a, b| b.cmp(a));
    assert_eq!(sorted, vec![100, 75, 50, 25]);
}

#[test]
fn fewer_than_four_top_performers_forfeits_unused_bucket() {
    let mut rf = PotBonus::new();
    let u1 = qualify_user(&mut rf);
    let _u2 = qualify_user(&mut rf);
    // u1 cycles more -> rank 1.
    let acct = Uuid::now_v7();
    rf.register_user_account(u1, acct);
    for _ in 0..5 {
        rf.record_matrix_cycle(acct, Uuid::now_v7()).unwrap();
    }
    rf.add_points(1000);

    let result = rf.distribute_weekly().unwrap();
    // Only 2 of the 4 top-performer slots are filled: 40% + 30% of 250 = 175.
    let total_top: u32 = result.top_cycler_distributions.iter().map(|(_, p)| p).sum();
    assert_eq!(
        total_top, 175,
        "remaining 75 of the 250 bucket is forfeited"
    );
}

#[test]
fn pool_resets_to_zero_after_distribution() {
    let mut rf = PotBonus::new();
    let _ = qualify_user(&mut rf);
    rf.add_points(10_000);
    let _ = rf.distribute_weekly().unwrap();
    assert_eq!(rf.total_pool_points(), 0, "no rollover");
}

#[test]
fn tracker_resets_after_distribution_allows_requalification_next_week() {
    let mut rf = PotBonus::new();
    let user = qualify_user(&mut rf);
    rf.add_points(10_000);
    let _ = rf.distribute_weekly().unwrap();

    assert!(
        !rf.is_user_qualified(&user),
        "no longer qualified after reset"
    );
    // A fresh graduation+cycle re-qualifies them for the next week.
    let acct = Uuid::now_v7();
    rf.register_user_account(user, acct);
    rf.record_graduation(acct).unwrap();
    rf.record_matrix_cycle(acct, Uuid::now_v7()).unwrap();
    assert!(rf.is_user_qualified(&user));
}

#[test]
fn custom_policy_changes_split_ratios() {
    let mut rf = PotBonus::with_policy(DistributionPolicy {
        profit_sharing_pct: 50, // 50/50 instead of 75/25
        top_performer_pct: 50,
        top_performer_shares: vec![100], // single winner takes all of the 50%
        max_top_performers: 1,
    });
    let user = qualify_user(&mut rf);
    rf.add_points(1000);
    let result = rf.distribute_weekly().unwrap();
    assert_eq!(result.profit_sharing_for(user), Some(500));
    assert_eq!(result.top_cycler_distributions[0].1, 500);
}
