//! Distribution policy and the immutable result of a weekly distribution.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Configurable distribution policy. The default is the canonical RFN model:
/// 75% profit sharing (equal split) + 25% to top performers on a
/// 40/30/20/10 ladder (top 4 only).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DistributionPolicy {
    /// Percentage of the pool for equal-split profit sharing.
    pub profit_sharing_pct: u32,
    /// Percentage of the pool for the top-performer bucket.
    pub top_performer_pct: u32,
    /// Percentage allocations of the top-performer bucket, applied to the top
    /// performers in ranked order (left-to-right). Default `[40, 30, 20, 10]`.
    pub top_performer_shares: Vec<u32>,
    /// Maximum number of top performers paid from the top-performer bucket.
    pub max_top_performers: usize,
}

impl Default for DistributionPolicy {
    fn default() -> Self {
        Self {
            profit_sharing_pct: 75,
            top_performer_pct: 25,
            top_performer_shares: vec![40, 30, 20, 10],
            max_top_performers: 4,
        }
    }
}

/// The immutable outcome of a weekly distribution.
#[derive(Debug, Clone, PartialEq)]
pub struct DistributionResult {
    /// Pool size at the moment of distribution.
    pub total_pool: u32,
    /// Sum of all points actually paid out (may be less than `total_pool` due
    /// to integer rounding and forfeited buckets).
    pub total_distributed: u32,
    /// `(user_id, points)` entries for the equal-split profit sharing.
    pub profit_sharing_distributions: Vec<(Uuid, u32)>,
    /// `(user_id, points)` entries for the top-performer bucket.
    pub top_cycler_distributions: Vec<(Uuid, u32)>,
}

impl DistributionResult {
    pub(crate) fn empty(pool: u32) -> Self {
        Self {
            total_pool: pool,
            total_distributed: 0,
            profit_sharing_distributions: Vec::new(),
            top_cycler_distributions: Vec::new(),
        }
    }

    /// Profit-sharing points paid to `user`, if any.
    pub fn profit_sharing_for(&self, user: Uuid) -> Option<u32> {
        self.profit_sharing_distributions
            .iter()
            .find(|(u, _)| *u == user)
            .map(|(_, p)| *p)
    }

    /// Top-performer points paid to `user`, if any.
    pub fn top_cycler_for(&self, user: Uuid) -> Option<u32> {
        self.top_cycler_distributions
            .iter()
            .find(|(u, _)| *u == user)
            .map(|(_, p)| *p)
    }
}
