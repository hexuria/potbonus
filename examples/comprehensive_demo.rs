//! # Comprehensive Demo
//!
//! Mirrors rfn's `examples/comprehensive_pot_bonus_demo.rs`: sets up users
//! with multiple accounts, exercises dual qualification, runs a 75-25 weekly
//! distribution, and prints the per-user breakdown.
//!
//! Run with: `cargo run --example comprehensive_demo`

use potbonus::PotBonus;
use uuid::Uuid;

fn main() {
    println!("=== potbonus comprehensive demo ===\n");

    let mut rf = PotBonus::new();
    let alice = Uuid::now_v7();
    let bob = Uuid::now_v7();
    let charlie = Uuid::now_v7();
    let diana = Uuid::now_v7();

    // Register accounts for each user.
    let alice_acct = Uuid::now_v7();
    let bob_acct = Uuid::now_v7();
    let charlie_acct = Uuid::now_v7();
    let diana_acct = Uuid::now_v7();
    rf.register_user_account(alice, alice_acct);
    rf.register_user_account(bob, bob_acct);
    rf.register_user_account(charlie, charlie_acct);
    rf.register_user_account(diana, diana_acct);

    // Alice: graduates AND cycles -> qualified.
    rf.record_graduation(alice_acct).unwrap();
    rf.record_matrix_cycle(alice_acct, Uuid::now_v7()).unwrap();
    // Bob: graduates but never cycles -> NOT qualified.
    rf.record_graduation(bob_acct).unwrap();
    // Charlie: cycles but never graduates -> NOT qualified.
    rf.record_matrix_cycle(charlie_acct, Uuid::now_v7())
        .unwrap();
    // Diana: graduates + 8 cycles -> qualified + top performer.
    rf.record_graduation(diana_acct).unwrap();
    for _ in 0..8 {
        rf.record_matrix_cycle(diana_acct, Uuid::now_v7()).unwrap();
    }

    println!("qualification status:");
    println!("  alice   (grad+cycle): {}", rf.is_user_qualified(&alice));
    println!("  bob     (grad only):  {}", rf.is_user_qualified(&bob));
    println!("  charlie (cycle only): {}", rf.is_user_qualified(&charlie));
    println!("  diana   (grad+8cyc):  {}", rf.is_user_qualified(&diana));

    rf.add_points(10_000);
    println!("\npool: 10000 points");
    let result = rf.distribute_weekly().unwrap();

    println!("\n=== distribution result ===");
    println!("total distributed: {} / 10000", result.total_distributed);
    println!("\nprofit sharing (75% = {}):", 10_000 * 75 / 100);
    for (uid, pts) in &result.profit_sharing_distributions {
        let name = user_name(*uid, alice, bob, charlie, diana);
        println!("  {name:<8} {pts}");
    }
    println!("\ntop performers (25% = {}):", 10_000 * 25 / 100);
    for (uid, pts) in &result.top_cycler_distributions {
        let name = user_name(*uid, alice, bob, charlie, diana);
        println!("  {name:<8} {pts}");
    }

    println!(
        "\npool after distribution: {} (no rollover)",
        rf.total_pool_points()
    );
    println!(
        "qualified after reset: alice={}, diana={}",
        rf.is_user_qualified(&alice),
        rf.is_user_qualified(&diana)
    );
}

fn user_name(uid: Uuid, alice: Uuid, bob: Uuid, charlie: Uuid, diana: Uuid) -> &'static str {
    if uid == alice {
        "alice"
    } else if uid == bob {
        "bob"
    } else if uid == charlie {
        "charlie"
    } else if uid == diana {
        "diana"
    } else {
        "unknown"
    }
}
