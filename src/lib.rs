//! # royalflush
//!
//! Weekly 75-25 pot bonus distribution with **dual qualification** for the
//! Royal Flush Network.
//!
//! ## Model
//!
//! Every week the pool is split:
//! - **`profit_sharing_pct`** (default **75%**) is divided **equally** among
//!   all qualified users. Remainder of the integer division is forfeited.
//! - **`top_performer_pct`** (default **25%**) goes to the top performers using
//!   the `top_performer_shares` percentage table (default
//!   `[40, 30, 20, 10]`), applied left-to-right. Unused slots (fewer top
//!   performers than shares) are forfeited.
//!
//! A user is **qualified** iff they have BOTH at least one flushline
//! graduation AND at least one matrix cycle, tracked at the *user* level
//! (a user may own many accounts). Cycles aggregate across all the user's
//! accounts.
//!
//! After distribution, the pool resets to **0** (no rollover) and the tracker
//! fully resets so users must re-qualify next week.
//!
//! ## Tier reset
//!
//! During distribution, every graduated account of every *qualified* user can
//! be reset to King tier via an injectable [`ResetPort`]. royalflush does NOT
//! depend on flushline — whoever wires the system provides the adapter.
//!
//! ## Events
//!
//! There is **no async runtime dependency**. The crate consumes
//! `FlushlineGraduated` / `MatrixCycled` payloads via explicit methods; it does
//! not own a channel.

mod pool;
mod reset;
mod tracker;

pub use pool::{DistributionPolicy, DistributionResult};
pub use reset::{ResetOutcome, ResetPort};
pub use tracker::{UserPerformance, UserQualification};

use uuid::Uuid;

/// The aggregate root: pool + qualification tracker + distribution policy.
pub struct RoyalFlush {
    pool_points: u32,
    tracker: tracker::UserQualificationTracker,
    policy: DistributionPolicy,
}

impl Default for RoyalFlush {
    fn default() -> Self {
        Self::new()
    }
}

impl RoyalFlush {
    /// Create with the default 75/25 + [40,30,20,10] policy.
    pub fn new() -> Self {
        Self {
            pool_points: 0,
            tracker: tracker::UserQualificationTracker::new(),
            policy: DistributionPolicy::default(),
        }
    }

    /// Create with a custom policy.
    pub fn with_policy(policy: DistributionPolicy) -> Self {
        Self {
            pool_points: 0,
            tracker: tracker::UserQualificationTracker::new(),
            policy,
        }
    }

    // ----- pool & registration ------------------------------------------

    /// Contribute points to the weekly pool.
    pub fn add_points(&mut self, points: u32) {
        self.pool_points += points;
    }

    /// Current pool size.
    pub fn total_pool_points(&self) -> u32 {
        self.pool_points
    }

    /// Map an account to its owning user.
    pub fn register_user_account(&mut self, user_id: Uuid, account_id: Uuid) {
        self.tracker.register_user_account(user_id, account_id);
    }

    // ----- event ingestion (typed Uuid, no String parsing) --------------

    /// Record a flushline graduation for `account_id`'s owning user.
    pub fn record_graduation(&mut self, account_id: Uuid) -> Result<(), String> {
        self.tracker.record_graduation(account_id)
    }

    /// Record a matrix cycle for `account_id`'s owning user.
    pub fn record_matrix_cycle(&mut self, account_id: Uuid, matrix_id: Uuid) -> Result<(), String> {
        self.tracker.record_matrix_cycle(account_id, matrix_id)
    }

    // ----- queries -------------------------------------------------------

    pub fn is_user_qualified(&self, user_id: &Uuid) -> bool {
        self.tracker.is_user_qualified(user_id)
    }

    pub fn user_cycle_count(&self, user_id: &Uuid) -> u32 {
        self.tracker.user_cycle_count(user_id)
    }

    pub fn qualified_users(&self) -> Vec<Uuid> {
        self.tracker.qualified_users()
    }

    pub fn top_performers(&self, limit: usize) -> Vec<UserPerformance> {
        self.tracker.top_performers(limit)
    }

    pub fn graduated_accounts_for_user(&self, user_id: &Uuid) -> Vec<Uuid> {
        self.tracker.graduated_accounts_for_user(user_id)
    }

    // ----- distribution --------------------------------------------------

    /// Run the weekly distribution (no tier reset). Returns the result and
    /// zeroes the pool + tracker.
    pub fn distribute_weekly(&mut self) -> Result<DistributionResult, NotEnoughPoints> {
        self.distribute_weekly_with_reset(None::<&mut reset::NullReset>)
            .map(|(r, _)| r)
    }

    /// Run the weekly distribution, optionally resetting graduated accounts of
    /// qualified users via `port`. Returns `(result, reset_outcome)`.
    pub fn distribute_weekly_with_reset<P>(
        &mut self,
        port: Option<&mut P>,
    ) -> Result<(DistributionResult, ResetOutcome), NotEnoughPoints>
    where
        P: ResetPort + ?Sized,
    {
        if self.pool_points == 0 {
            return Err(NotEnoughPoints);
        }

        let qualified = self.tracker.qualified_users();
        if qualified.is_empty() {
            // No qualified users: nothing to distribute; pool preserved.
            return Ok((
                DistributionResult::empty(self.pool_points),
                ResetOutcome::Skipped,
            ));
        }

        let profit_total =
            (self.pool_points as f32 * self.policy.profit_sharing_pct as f32 / 100.0) as u32;
        let top_total =
            (self.pool_points as f32 * self.policy.top_performer_pct as f32 / 100.0) as u32;

        // 1. Profit sharing: equal split among qualified users.
        let per_user = profit_total / qualified.len() as u32;
        let profit_sharing: Vec<(Uuid, u32)> = qualified.iter().map(|u| (*u, per_user)).collect();

        // 2. Top performers: top-N by cycle count (tie-break graduations),
        //    distributed by the policy's percentage table.
        let top = self.tracker.top_performers(self.policy.max_top_performers);
        let top_cycler: Vec<(Uuid, u32)> = top
            .iter()
            .enumerate()
            .zip(self.policy.top_performer_shares.iter())
            .map(|((_, perf), pct)| {
                let points = (top_total as f32 * *pct as f32 / 100.0) as u32;
                (perf.user_id, points)
            })
            .collect();

        let total_distributed: u32 = profit_sharing.iter().map(|(_, p)| p).sum::<u32>()
            + top_cycler.iter().map(|(_, p)| p).sum::<u32>();

        // 3. Snapshot graduated accounts of qualified users BEFORE tracker reset.
        let to_reset: Vec<(Uuid, String)> = qualified
            .iter()
            .flat_map(|u| {
                self.tracker
                    .graduated_accounts_for_user(u)
                    .into_iter()
                    .map(|a| (a, format!("user-{a}")))
            })
            .collect();

        // 4. Reset tracker + pool (no rollover).
        self.tracker.reset_weekly();
        let original_pool = self.pool_points;
        self.pool_points = 0;

        // 5. Apply tier reset via the port, if provided.
        let reset_outcome = match port {
            None => ResetOutcome::Skipped,
            Some(port) => {
                if to_reset.is_empty() {
                    ResetOutcome::All(0)
                } else {
                    let mut succeeded = 0u32;
                    let mut failed = 0u32;
                    for (acct, owner) in to_reset {
                        match port.reset_to_king(acct, owner) {
                            Ok(()) => succeeded += 1,
                            Err(_) => failed += 1,
                        }
                    }
                    if failed == 0 {
                        ResetOutcome::All(succeeded)
                    } else {
                        ResetOutcome::Partial { succeeded, failed }
                    }
                }
            }
        };

        Ok((
            DistributionResult {
                total_pool: original_pool,
                total_distributed,
                profit_sharing_distributions: profit_sharing,
                top_cycler_distributions: top_cycler,
            },
            reset_outcome,
        ))
    }
}

/// Error returned when distribution is attempted on an empty pool with
/// qualified users present (matches rfn's `PotBonusError::NotEnoughPoints`).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("not enough points in the pool for distribution")]
pub struct NotEnoughPoints;
