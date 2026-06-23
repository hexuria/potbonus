//! # Edge Cases Demo
//!
//! Mirrors rfn's `examples/edge_cases_demo.rs`: empty pool, single qualified
//! user, ties, whale dominance, no qualifiers, and graceful reset failure.
//!
//! Run with: `cargo run --example edge_cases`

use potbonus::{PotBonus, ResetOutcome, ResetPort};
use uuid::Uuid;

struct FailingReset;
impl ResetPort for FailingReset {
    fn reset_to_king(&mut self, _account_id: Uuid, _owner: String) -> Result<(), String> {
        Err("forced failure".into())
    }
}

fn qualify(rf: &mut PotBonus, cycles: u32) -> Uuid {
    let user = Uuid::now_v7();
    let acct = Uuid::now_v7();
    rf.register_user_account(user, acct);
    rf.record_graduation(acct).unwrap();
    for _ in 0..cycles.max(1) {
        rf.record_matrix_cycle(acct, Uuid::now_v7()).unwrap();
    }
    user
}

fn main() {
    println!("=== potbonus edge cases ===\n");

    // 1. Empty pool with a qualified user -> error.
    let mut rf = PotBonus::new();
    let _ = qualify(&mut rf, 1);
    println!(
        "1. empty pool distribution: {:?}",
        rf.distribute_weekly().err().map(|_| "NotEnoughPoints")
    );

    // 2. Single qualified user gets both the 75% and the 25% (rank-1 of top-4).
    let mut rf = PotBonus::new();
    let only = qualify(&mut rf, 1);
    rf.add_points(1000);
    let result = rf.distribute_weekly().unwrap();
    println!(
        "2. single qualified user: profit={}, top={} (effectively the whole pool)",
        result.profit_sharing_for(only).unwrap(),
        result.top_cycler_for(only).unwrap_or(0),
    );

    // 3. Six perfectly-tied users (5 cycles each) -> top-4 deterministic by ranking.
    let mut rf = PotBonus::new();
    let users: Vec<_> = (0..6).map(|_| qualify(&mut rf, 5)).collect();
    rf.add_points(1000);
    let result = rf.distribute_weekly().unwrap();
    let tied_top: Vec<_> = users
        .iter()
        .filter_map(|u| result.top_cycler_for(*u))
        .collect();
    println!(
        "3. six tied users: {} got top-performer payouts (40/30/20/10 of 250)",
        tied_top.len()
    );

    // 4. Whale (100 cycles) vs normals.
    let mut rf = PotBonus::new();
    let n1 = qualify(&mut rf, 1);
    let _n2 = qualify(&mut rf, 1);
    let whale = qualify(&mut rf, 100);
    rf.add_points(1000);
    let result = rf.distribute_weekly().unwrap();
    println!(
        "4. whale dominance: normal={}, whale top-performer={}",
        result.profit_sharing_for(n1).unwrap(),
        result.top_cycler_for(whale).unwrap(),
    );

    // 5. No qualified users -> no distribution, pool preserved.
    let mut rf = PotBonus::new();
    rf.add_points(1000);
    let result = rf.distribute_weekly().unwrap();
    println!(
        "5. no qualifiers: distributed {}, pool preserved at {}",
        result.total_distributed,
        rf.total_pool_points()
    );

    // 6. Reset failure is reported but distribution still completes.
    let mut rf = PotBonus::new();
    let _u = qualify(&mut rf, 1);
    rf.add_points(1000);
    let mut failing = FailingReset;
    let (_result, outcome) = rf.distribute_weekly_with_reset(Some(&mut failing)).unwrap();
    println!(
        "6. all resets fail: outcome = {:?}, pool = {}",
        outcome,
        rf.total_pool_points()
    );
    let _ = ResetOutcome::Skipped;
}
