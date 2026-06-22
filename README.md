# royalflush

Weekly **75-25 pot bonus** distribution with **dual qualification** for the
Royal Flush Network.

## Model

Every week the pool is split:

| Bucket                | Default | Rule                                                     |
|-----------------------|---------|----------------------------------------------------------|
| Profit sharing        | **75%** | Equal split among all qualified users.                   |
| Top performer         | **25%** | Distributed to the top performers by a percentage table. |

Default top-performer table: `[40, 30, 20, 10]` of the 25% bucket, applied to
the top **4** users (ranked by cycle count, tie-broken by graduation count).
Unused slots (fewer top performers than table entries) are **forfeited**.
Remainders from integer division are also forfeited.

After distribution the pool resets to **0** (no rollover) and the tracker
fully resets, so users must re-qualify for the next week.

## Dual qualification

A user is **qualified** iff they have **both**:

1. At least one flushline graduation, **and**
2. At least one matrix cycle.

Qualification is tracked at the **user** level (a user may own many accounts).
The two requirements may be satisfied by **different** accounts the user owns.
Cycles aggregate across all the user's accounts.

## Configurable policy

The split ratios and top-performer table are configurable via
[`DistributionPolicy`]:

```rust
use royalflush::{DistributionPolicy, RoyalFlush};

let rf = RoyalFlush::with_policy(DistributionPolicy {
    profit_sharing_pct: 50,           // 50/50 instead of 75/25
    top_performer_pct: 50,
    top_performer_shares: vec![100],  // single winner takes the bucket
    max_top_performers: 1,
});
```

## Tier reset (integration port)

During distribution, every graduated account of every qualified user may be
reset to King tier via an injectable [`ResetPort`]. royalflush does **NOT**
depend on flushline — whoever wires the system provides the adapter:

```rust
use royalflush::{ResetPort, RoyalFlush};

struct FlushlineAdapter { /* delegates to flushline */ }
impl ResetPort for FlushlineAdapter {
    fn reset_to_king(&mut self, account_id, owner) -> Result<(), String> { /* ... */ }
}

let (result, outcome) = rf.distribute_weekly_with_reset(Some(&mut adapter))?;
```

Reset failures are reported in [`ResetOutcome`] but do **not** roll back the
distribution.

## Events

No async runtime dependency. The crate consumes `FlushlineGraduated` /
`MatrixCycled` payloads via explicit methods (`record_graduation`,
`record_matrix_cycle`) — no channels.

## Usage

```toml
[dependencies]
royalflush = "0.1"
```

## Testing & verification

```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo run --example comprehensive_demo
cargo run --example weekly_cycle
cargo run --example edge_cases
```

## Related crates

- [`flushevents`](../flushevents) — shared event payloads.
- [`flushline`](../flushline) — 5-tier card progression engine (provides the
  graduation events royalflush consumes, and the `reset_to_king` adapter target).
- [`flushmatrix`](../flushmatrix) — 2×3 forced-matrix referral tree (provides
  the matrix cycle events royalflush consumes).
