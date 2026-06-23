//! # potbonus
//!
//! Weekly **75-25 pot bonus** distribution with **dual qualification** for the
//! Royal Flush Network.
//!
//! This crate is pure domain logic — **no database, no async runtime, no
//! network**. It does not depend on any other RFN crate. A caller wires it up
//! by feeding it graduation/cycle events and (optionally) providing a
//! [`ResetPort`] adapter.
//!
//! ## Model
//!
//! Every week the pool is split (see [`DistributionPolicy`]):
//!
//! - **75% (default)** → **equal split** among all qualified users. The
//!   remainder of the integer division is forfeited.
//! - **25% (default)** → **top performers**, distributed using the
//!   `top_performer_shares` table (default `[40, 30, 20, 10]`), applied
//!   left-to-right to the top-ranked users. Unused slots (fewer top performers
//!   than table entries) are forfeited.
//!
//! ## Dual qualification
//!
//! A user is **qualified** iff they have **both** at least one graduation AND
//! at least one matrix cycle. Qualification is tracked at the *user* level (a
//! user may own many accounts; the two requirements may be satisfied by
//! different accounts). Cycles aggregate across all the user's accounts.
//!
//! After distribution the pool resets to **0** (no rollover) and the tracker
//! fully resets, so users must re-qualify for the next week.
//!
//! ## Tier reset (integration port)
//!
//! During distribution, every graduated account of every *qualified* user may
//! be reset to King tier via an injectable [`ResetPort`]. potbonus does NOT
//! depend on flushline — whoever wires the system provides the adapter. Reset
//! failures are reported in [`ResetOutcome`] but do not roll back the
//! distribution.
//!
//! # Quick start
//!
//! ```
//! use potbonus::PotBonus;
//! use uuid::Uuid;
//!
//! let mut rf = PotBonus::new();
//!
//! // A user owns an account; qualify them with a graduation + a matrix cycle.
//! let user = Uuid::now_v7();
//! let acct = Uuid::now_v7();
//! rf.register_user_account(user, acct);
//! rf.record_graduation(acct).unwrap();
//! rf.record_matrix_cycle(acct, Uuid::now_v7()).unwrap();
//! assert!(rf.is_user_qualified(&user));
//!
//! // Fund the weekly pool and distribute.
//! rf.add_points(1_000);
//! let result = rf.distribute_weekly().unwrap();
//!
//! // 75% -> 750 to the single qualified user; 25% -> 250 to the top performer.
//! assert_eq!(result.profit_sharing_for(user), Some(750));
//! assert_eq!(result.top_cycler_for(user), Some(100)); // 40% of 250
//! assert_eq!(rf.total_pool_points(), 0); // no rollover
//! ```

mod pool;
mod reset;
mod tracker;

#[cfg(feature = "db")]
pub mod repository;

pub use pool::{DistributionPolicy, DistributionResult};
pub use reset::{ResetOutcome, ResetPort};
pub use tracker::{UserPerformance, UserQualification};

#[cfg(feature = "db")]
pub use repository::{PgPotBonusRepository, PotBonusRepository};

use uuid::Uuid;

/// The aggregate root: pool + qualification tracker + distribution policy.
pub struct PotBonus {
    pub(crate) pool_points: u32,
    pub(crate) tracker: tracker::UserQualificationTracker,
    pub(crate) policy: DistributionPolicy,
}

impl Default for PotBonus {
    fn default() -> Self {
        Self::new()
    }
}

impl PotBonus {
    /// Create with the default policy: 75/25 split, top-performer table
    /// `[40, 30, 20, 10]`, top 4 only.
    pub fn new() -> Self {
        Self {
            pool_points: 0,
            tracker: tracker::UserQualificationTracker::new(),
            policy: DistributionPolicy::default(),
        }
    }

    /// Create with a custom [`DistributionPolicy`] (e.g. a 50/50 split or a
    /// single-winner top-performer table).
    pub fn with_policy(policy: DistributionPolicy) -> Self {
        Self {
            pool_points: 0,
            tracker: tracker::UserQualificationTracker::new(),
            policy,
        }
    }

    // ----- pool & registration ------------------------------------------

    /// Contribute `points` to the weekly pool. Points accumulate until the next
    /// [`Self::distribute_weekly`] call zeroes the pool.
    pub fn add_points(&mut self, points: u32) {
        self.pool_points += points;
    }

    /// Current pool size.
    pub fn total_pool_points(&self) -> u32 {
        self.pool_points
    }

    /// Map an `account_id` to its owning `user_id`. A user may own many
    /// accounts; call this once per account before recording events for it.
    pub fn register_user_account(&mut self, user_id: Uuid, account_id: Uuid) {
        self.tracker.register_user_account(user_id, account_id);
    }

    // ----- event ingestion (typed Uuid, no String parsing) --------------

    /// Record a flushline graduation for `account_id`'s owning user. This is
    /// one of the **two** qualification requirements.
    ///
    /// Returns `Err` if `account_id` was never registered via
    /// [`Self::register_user_account`].
    pub fn record_graduation(&mut self, account_id: Uuid) -> Result<(), String> {
        self.tracker.record_graduation(account_id)
    }

    /// Record a matrix cycle for `account_id`'s owning user (`matrix_id`
    /// identifies which matrix cycled). This is one of the **two**
    /// qualification requirements and also bumps the user's cycle count (used
    /// for top-performer ranking).
    ///
    /// Returns `Err` if `account_id` was never registered via
    /// [`Self::register_user_account`].
    pub fn record_matrix_cycle(&mut self, account_id: Uuid, matrix_id: Uuid) -> Result<(), String> {
        self.tracker.record_matrix_cycle(account_id, matrix_id)
    }

    // ----- queries -------------------------------------------------------

    /// `true` if the user has both a graduation and a matrix cycle on record.
    pub fn is_user_qualified(&self, user_id: &Uuid) -> bool {
        self.tracker.is_user_qualified(user_id)
    }

    /// Total cycles across all the user's accounts (used for ranking).
    pub fn user_cycle_count(&self, user_id: &Uuid) -> u32 {
        self.tracker.user_cycle_count(user_id)
    }

    /// All currently-qualified user ids.
    pub fn qualified_users(&self) -> Vec<Uuid> {
        self.tracker.qualified_users()
    }

    /// Top performers ranked by cycle count (tie-break: graduation count),
    /// truncated to `limit`. Only qualified users are ranked.
    pub fn top_performers(&self, limit: usize) -> Vec<UserPerformance> {
        self.tracker.top_performers(limit)
    }

    /// All graduated account ids owned by `user_id`.
    pub fn graduated_accounts_for_user(&self, user_id: &Uuid) -> Vec<Uuid> {
        self.tracker.graduated_accounts_for_user(user_id)
    }

    // ----- distribution --------------------------------------------------

    /// Run the weekly distribution **without** tier reset.
    ///
    /// Splits the pool per the policy, zeroes the pool, and fully resets the
    /// tracker. Returns [`NotEnoughPoints`] only if the pool is empty **and**
    /// at least one user is qualified (an empty pool with no qualifiers
    /// succeeds with a zero-distribution result and preserves the pool).
    pub fn distribute_weekly(&mut self) -> Result<DistributionResult, NotEnoughPoints> {
        self.distribute_weekly_with_reset(None::<&mut reset::NullReset>)
            .map(|(r, _)| r)
    }

    /// Run the weekly distribution, optionally resetting graduated accounts of
    /// qualified users via `port`. Pass `None` to skip the reset phase.
    ///
    /// Returns `(result, reset_outcome)`. Reset failures are captured in
    /// [`ResetOutcome::Partial`] but do **not** roll back the distribution.
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
