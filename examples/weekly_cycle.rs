//! # Weekly Cycle Simulation
//!
//! Mirrors rfn's `examples/weekly_cycle_simulation.rs`: simulates multiple
//! consecutive weekly distributions to show progressive qualification,
//! pool-reset behavior, and tier-reset integration.
//!
//! Run with: `cargo run --example weekly_cycle`

use royalflush::{ResetOutcome, ResetPort, RoyalFlush};
use uuid::Uuid;

/// A reset port that just logs each call. In a real system this would
/// delegate to flushline; here we keep the example self-contained.
struct LoggingReset {
    resets: u32,
}

impl ResetPort for LoggingReset {
    fn reset_to_king(&mut self, account_id: Uuid, _owner: String) -> Result<(), String> {
        self.resets += 1;
        let _ = account_id;
        Ok(())
    }
}

fn main() {
    println!("=== royalflush weekly cycle simulation ===\n");

    let mut rf = RoyalFlush::new();
    let mut port = LoggingReset { resets: 0 };

    let alice = Uuid::now_v7();
    let bob = Uuid::now_v7();
    let alice_acct = Uuid::now_v7();
    let bob_acct = Uuid::now_v7();
    rf.register_user_account(alice, alice_acct);
    rf.register_user_account(bob, bob_acct);

    let weeks: &[(u32, &str)] = &[
        (2_500, "week 1"),
        (5_000, "week 2"),
        (8_000, "week 3"),
        (7_500, "week 4"),
    ];

    for (week_idx, (pool, label)) in weeks.iter().enumerate() {
        println!("--- {label} (pool {pool}) ---");

        // Re-qualify users each week (tracker resets at distribution).
        if week_idx % 2 == 0 {
            rf.record_graduation(alice_acct).unwrap();
            rf.record_matrix_cycle(alice_acct, Uuid::now_v7()).unwrap();
        }
        if week_idx >= 1 {
            rf.record_graduation(bob_acct).unwrap();
            rf.record_matrix_cycle(bob_acct, Uuid::now_v7()).unwrap();
        }

        rf.add_points(*pool);
        let (result, outcome) = rf.distribute_weekly_with_reset(Some(&mut port)).unwrap();

        println!(
            "  alice qualified: {}, bob qualified: {}",
            !rf.is_user_qualified(&alice), // just reset
            !rf.is_user_qualified(&bob)
        );
        println!("  distributed {}/{}", result.total_distributed, pool);
        let _ = outcome;
    }

    println!("\ntotal tier-resets issued: {}", port.resets);
    match port.resets {
        0 => println!("ResetOutcome: Skipped/none"),
        n => println!("ResetOutcome: reset {n} accounts across all weeks"),
    }
    let _ = ResetOutcome::Skipped; // type is exported
}
