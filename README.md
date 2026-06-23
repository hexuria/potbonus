# potbonus

**Weekly 75-25 pot bonus** distribution with **dual qualification** for the
Royal Flush Network (RFN).

Every week a points pool is split: 75% goes equally to all qualified users,
25% goes to the top performers on a `[40, 30, 20, 10]` ladder. After
distribution the pool resets to 0 and users must re-qualify for the next week.

This crate is pure domain logic — **no database, no async runtime, no network**.
It does not depend on any other RFN crate.

## Model

| Bucket          | Default | Rule                                                       |
|-----------------|---------|------------------------------------------------------------|
| Profit sharing  | **75%** | Equal split among all qualified users.                     |
| Top performer   | **25%** | Distributed to the top performers by a percentage table.   |

Default top-performer table: `[40, 30, 20, 10]` of the 25% bucket, applied to
the top **4** users (ranked by cycle count, tie-broken by graduation count).
Unused slots (fewer top performers than table entries) are **forfeited**.
Remainders from integer division are also forfeited.

## Dual qualification

A user is **qualified** iff they have **both**:

1. At least one graduation, **and**
2. At least one matrix cycle.

Qualification is tracked at the **user** level (a user may own many accounts).
The two requirements may be satisfied by **different** accounts the user owns.
Cycles aggregate across all the user's accounts.

## Quick start

```rust
use potbonus::PotBonus;
use uuid::Uuid;

let mut rf = PotBonus::new();

// A user owns an account; qualify them with a graduation + a matrix cycle.
let user = Uuid::now_v7();
let acct = Uuid::now_v7();
rf.register_user_account(user, acct);
rf.record_graduation(acct).unwrap();
rf.record_matrix_cycle(acct, Uuid::now_v7()).unwrap();
assert!(rf.is_user_qualified(&user));

// Fund the weekly pool and distribute.
rf.add_points(1_000);
let result = rf.distribute_weekly().unwrap();

// 75% -> 750 to the single qualified user; 25% -> 250, of which 40% = 100 to
// the top performer (the only one).
assert_eq!(result.profit_sharing_for(user), Some(750));
assert_eq!(result.top_cycler_for(user), Some(100));
assert_eq!(rf.total_pool_points(), 0); // no rollover
```

## Configurable policy

The split ratios and top-performer table are configurable via
`DistributionPolicy`:

```rust
use potbonus::{DistributionPolicy, PotBonus};

let rf = PotBonus::with_policy(DistributionPolicy {
    profit_sharing_pct: 50,           // 50/50 instead of 75/25
    top_performer_pct: 50,
    top_performer_shares: vec![100],  // single winner takes the whole bucket
    max_top_performers: 1,
});
```

## Tier reset (integration port)

During distribution, every graduated account of every qualified user may be
reset to a lower tier (e.g. King) via an injectable `ResetPort`. potbonus
does **NOT** depend on any tier-management crate — whoever wires the system
provides the adapter:

```rust
use potbonus::{ResetOutcome, ResetPort, PotBonus};
use uuid::Uuid;

// Your adapter — e.g. one that delegates to your card-progression engine.
struct TierAdapter;
impl ResetPort for TierAdapter {
    fn reset_to_king(&mut self, account_id: Uuid, owner: String) -> Result<(), String> {
        // ...delegate to your engine...
        Ok(())
    }
}

let mut rf = PotBonus::new();
// ...qualify users, add_points...
let mut adapter = TierAdapter;
let (result, outcome) = rf.distribute_weekly_with_reset(Some(&mut adapter)).unwrap();
match outcome {
    ResetOutcome::All(n) => println!("reset {n} accounts"),
    ResetOutcome::Partial { succeeded, failed } => {
        eprintln!("reset {succeeded}, failed {failed} (distribution still applied)");
    }
    ResetOutcome::Skipped => println!("no reset port supplied"),
}
```

Reset failures are reported in `ResetOutcome::Partial` but do **not** roll back
the distribution — the pool is still zeroed and the tracker still reset.

## Events

No async runtime dependency. The crate consumes graduation/cycle events via
explicit typed methods (`record_graduation`, `record_matrix_cycle`) — no
channels, no string parsing.

## Examples

```bash
cargo run --example comprehensive_demo   # 4 users, dual qualification, 75-25 split
cargo run --example weekly_cycle         # 4 consecutive weekly distributions
cargo run --example edge_cases           # empty pool, ties, whale, reset failure
cargo doc --no-deps --open               # browse the rustdoc
```

## Testing & verification

```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test                                # 24 tests
```

## WebAssembly (WASM) & WASI Support

`potbonus` is fully compatible with WebAssembly **out of the box**. It supports compilation for both browser environments (Leptos frontend clients) and server-side WASM sandboxes (such as **Leptos Spin** or **Leptos Wasmtime**).

### 1. Browser-Side WebAssembly (`wasm32-unknown-unknown`)
Pre-configured with `uuid/js` feature enabled, so generating secure `v7` UUIDs requests secure entropy from browser-native JavaScript APIs (`window.crypto.getRandomValues`).
```bash
cargo check --target wasm32-unknown-unknown
```

### 2. Server-Side WASM / WASI (`wasm32-wasip1`)
Compiles seamlessly to WASI for deployments like Spin and Wasmtime. WASI system calls provide entropy natively.
```bash
cargo check --target wasm32-wasip1
```

## License

MIT.

