//! User-level qualification tracking. A user may own many accounts; both the
//! flushline graduation and the matrix cycle requirements are tracked at the
//! *user* level.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Per-user qualification state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserQualification {
    pub user_id: Uuid,
    pub graduations: Vec<Uuid>,
    pub cycles: Vec<CycleRecord>,
    pub total_cycle_count: u32,
    pub is_qualified: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CycleRecord {
    pub account_id: Uuid,
    pub matrix_id: Uuid,
}

/// Snapshot of a user's performance used for top-performer ranking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserPerformance {
    pub user_id: Uuid,
    pub cycle_count: u32,
    pub graduation_count: u32,
}

pub(crate) struct UserQualificationTracker {
    pub(crate) qualifications: HashMap<Uuid, UserQualification>,
    pub(crate) account_to_user: HashMap<Uuid, Uuid>,
}

impl UserQualificationTracker {
    pub(crate) fn new() -> Self {
        Self {
            qualifications: HashMap::new(),
            account_to_user: HashMap::new(),
        }
    }

    pub(crate) fn register_user_account(&mut self, user_id: Uuid, account_id: Uuid) {
        self.account_to_user.insert(account_id, user_id);
        self.qualifications
            .entry(user_id)
            .or_insert_with(|| UserQualification {
                user_id,
                graduations: Vec::new(),
                cycles: Vec::new(),
                total_cycle_count: 0,
                is_qualified: false,
            });
    }

    pub(crate) fn record_graduation(&mut self, account_id: Uuid) -> Result<(), String> {
        let user_id = *self
            .account_to_user
            .get(&account_id)
            .ok_or_else(|| format!("no user owns account {account_id}"))?;
        let q = self
            .qualifications
            .get_mut(&user_id)
            .expect("qualification exists once an account is registered");
        q.graduations.push(account_id);
        self.recompute(&user_id);
        Ok(())
    }

    pub(crate) fn record_matrix_cycle(
        &mut self,
        account_id: Uuid,
        matrix_id: Uuid,
    ) -> Result<(), String> {
        let user_id = *self
            .account_to_user
            .get(&account_id)
            .ok_or_else(|| format!("no user owns account {account_id}"))?;
        let q = self
            .qualifications
            .get_mut(&user_id)
            .expect("qualification exists once an account is registered");
        q.cycles.push(CycleRecord {
            account_id,
            matrix_id,
        });
        q.total_cycle_count += 1;
        self.recompute(&user_id);
        Ok(())
    }

    fn recompute(&mut self, user_id: &Uuid) {
        if let Some(q) = self.qualifications.get_mut(user_id) {
            q.is_qualified = !q.graduations.is_empty() && !q.cycles.is_empty();
        }
    }

    pub(crate) fn is_user_qualified(&self, user_id: &Uuid) -> bool {
        self.qualifications
            .get(user_id)
            .map(|q| q.is_qualified)
            .unwrap_or(false)
    }

    pub(crate) fn user_cycle_count(&self, user_id: &Uuid) -> u32 {
        self.qualifications
            .get(user_id)
            .map(|q| q.total_cycle_count)
            .unwrap_or(0)
    }

    pub(crate) fn qualified_users(&self) -> Vec<Uuid> {
        self.qualifications
            .values()
            .filter(|q| q.is_qualified)
            .map(|q| q.user_id)
            .collect()
    }

    pub(crate) fn top_performers(&self, limit: usize) -> Vec<UserPerformance> {
        let mut perf: Vec<UserPerformance> = self
            .qualifications
            .values()
            .filter(|q| q.is_qualified)
            .map(|q| UserPerformance {
                user_id: q.user_id,
                cycle_count: q.total_cycle_count,
                graduation_count: q.graduations.len() as u32,
            })
            .collect();
        // Rank: cycles desc, tie-break graduations desc.
        perf.sort_by(|a, b| {
            b.cycle_count
                .cmp(&a.cycle_count)
                .then_with(|| b.graduation_count.cmp(&a.graduation_count))
        });
        perf.truncate(limit);
        perf
    }

    pub(crate) fn graduated_accounts_for_user(&self, user_id: &Uuid) -> Vec<Uuid> {
        self.qualifications
            .get(user_id)
            .map(|q| q.graduations.clone())
            .unwrap_or_default()
    }

    /// Full reset: every user reverts to unqualified with zeroed counters
    /// (matching rfn's `reset_weekly`).
    pub(crate) fn reset_weekly(&mut self) {
        for q in self.qualifications.values_mut() {
            q.graduations.clear();
            q.cycles.clear();
            q.total_cycle_count = 0;
            q.is_qualified = false;
        }
    }
}
