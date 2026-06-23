//! The tier-reset port: the integration seam with flushline. potbonus does
//! NOT depend on flushline; whoever wires the system provides an adapter that
//! implements [`ResetPort`].

use uuid::Uuid;

/// Outcome of the tier-reset phase of a weekly distribution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResetOutcome {
    /// No reset port was supplied; reset was skipped.
    Skipped,
    /// Every targeted account reset successfully (count included).
    All(u32),
    /// Some accounts reset, some failed. Distribution is NOT rolled back.
    Partial { succeeded: u32, failed: u32 },
}

/// Port implemented by an external tier-management system (e.g. flushline) to
/// reset a graduated account back to King tier.
///
/// Returning `Err` from `reset_to_king` reports a failed reset but does NOT
/// abort the distribution; the caller records the failure in [`ResetOutcome`].
pub trait ResetPort {
    /// Reset `account_id` (owned by `owner`) to King tier.
    fn reset_to_king(&mut self, account_id: Uuid, owner: String) -> Result<(), String>;
}

/// A no-op port used to satisfy the `?Sized` bound when callers pass
/// `None::<&mut T>` to [`crate::PotBonus::distribute_weekly_with_reset`].
/// Not part of the public API.
#[doc(hidden)]
pub struct NullReset;

impl ResetPort for NullReset {
    fn reset_to_king(&mut self, _account_id: Uuid, _owner: String) -> Result<(), String> {
        Ok(())
    }
}
