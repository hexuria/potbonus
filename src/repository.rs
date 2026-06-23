//! Repository for persisting and loading PotBonus aggregates to/from PostgreSQL.

use crate::tracker::{CycleRecord, UserQualificationTracker};
use crate::PotBonus;
use async_trait::async_trait;
use sqlx::{PgPool, Row};
use uuid::Uuid;

/// Repository interface for PotBonus aggregate persistence.
#[async_trait]
pub trait PotBonusRepository: Send + Sync {
    /// Load the complete PotBonus state from the database.
    async fn load(&self) -> Result<PotBonus, String>;

    /// Save the complete PotBonus state to the database transactionally.
    async fn save(&self, pot_bonus: &PotBonus) -> Result<(), String>;
}

/// Postgres-backed implementation of [`PotBonusRepository`].
#[derive(Debug, Clone)]
pub struct PgPotBonusRepository {
    pool: PgPool,
}

impl PgPotBonusRepository {
    /// Create a new PostgreSQL repository instance.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PotBonusRepository for PgPotBonusRepository {
    async fn load(&self) -> Result<PotBonus, String> {
        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| format!("Failed to acquire DB connection: {e}"))?;

        // 1. Fetch global pool points
        let state_row = sqlx::query("SELECT pool_points FROM pot_bonus_state WHERE id = 1")
            .fetch_optional(&mut *conn)
            .await
            .map_err(|e| format!("Failed to fetch global state: {e}"))?;

        let pool_points = match state_row {
            Some(row) => {
                let pts: i32 = row.get("pool_points");
                pts as u32
            }
            None => 0,
        };

        // 2. Fetch persistent account-to-user registrations
        let registrations_rows =
            sqlx::query("SELECT account_id, user_id FROM pot_bonus_registrations")
                .fetch_all(&mut *conn)
                .await
                .map_err(|e| format!("Failed to fetch registrations: {e}"))?;

        // 3. Fetch weekly graduations
        let graduations_rows =
            sqlx::query("SELECT account_id, user_id FROM pot_bonus_weekly_graduations")
                .fetch_all(&mut *conn)
                .await
                .map_err(|e| format!("Failed to fetch weekly graduations: {e}"))?;

        // 4. Fetch weekly cycles
        let cycles_rows =
            sqlx::query("SELECT account_id, matrix_id, user_id FROM pot_bonus_weekly_cycles")
                .fetch_all(&mut *conn)
                .await
                .map_err(|e| format!("Failed to fetch weekly cycles: {e}"))?;

        // 5. Hydrate UserQualificationTracker
        let mut tracker = UserQualificationTracker::new();

        // 5.1. Hydrate registrations
        for r in registrations_rows {
            let acct_uuid: Uuid = r.get("account_id");
            let user_uuid: Uuid = r.get("user_id");
            tracker.register_user_account(user_uuid, acct_uuid);
        }

        // 5.2. Hydrate weekly graduations
        for r in graduations_rows {
            let acct_uuid: Uuid = r.get("account_id");
            let user_uuid: Uuid = r.get("user_id");
            if let Some(q) = tracker.qualifications.get_mut(&user_uuid) {
                q.graduations.push(acct_uuid);
            }
        }

        // 5.3. Hydrate weekly cycles
        for r in cycles_rows {
            let acct_uuid: Uuid = r.get("account_id");
            let matrix_uuid: Uuid = r.get("matrix_id");
            let user_uuid: Uuid = r.get("user_id");
            if let Some(q) = tracker.qualifications.get_mut(&user_uuid) {
                q.cycles.push(CycleRecord {
                    account_id: acct_uuid,
                    matrix_id: matrix_uuid,
                });
                q.total_cycle_count += 1;
            }
        }

        // 5.4. Recompute qualification statuses
        for q in tracker.qualifications.values_mut() {
            q.is_qualified = !q.graduations.is_empty() && !q.cycles.is_empty();
        }

        Ok(PotBonus {
            pool_points,
            tracker,
            policy: crate::DistributionPolicy::default(),
        })
    }

    async fn save(&self, pot_bonus: &PotBonus) -> Result<(), String> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| format!("Failed to begin transaction: {e}"))?;

        // 1. Upsert global state
        sqlx::query(
            "INSERT INTO pot_bonus_state (id, pool_points) \
             VALUES (1, $1) \
             ON CONFLICT (id) DO UPDATE SET pool_points = EXCLUDED.pool_points, updated_at = NOW()",
        )
        .bind(pot_bonus.pool_points as i32)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to upsert state: {e}"))?;

        // 2. Upsert account-to-user registrations
        for (&acct, &user) in &pot_bonus.tracker.account_to_user {
            sqlx::query(
                "INSERT INTO pot_bonus_registrations (account_id, user_id) \
                 VALUES ($1, $2) \
                 ON CONFLICT (account_id) DO NOTHING",
            )
            .bind(acct)
            .bind(user)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Failed to insert registration: {e}"))?;
        }

        // 3. Clear existing weekly graduations and cycles
        sqlx::query("DELETE FROM pot_bonus_weekly_graduations")
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Failed to clear weekly graduations: {e}"))?;

        sqlx::query("DELETE FROM pot_bonus_weekly_cycles")
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Failed to clear weekly cycles: {e}"))?;

        // 4. Save current weekly graduations
        for q in pot_bonus.tracker.qualifications.values() {
            for &acct in &q.graduations {
                sqlx::query(
                    "INSERT INTO pot_bonus_weekly_graduations (account_id, user_id) \
                     VALUES ($1, $2) \
                     ON CONFLICT (account_id) DO NOTHING",
                )
                .bind(acct)
                .bind(q.user_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("Failed to insert graduation record: {e}"))?;
            }
        }

        // 5. Save current weekly cycles
        for q in pot_bonus.tracker.qualifications.values() {
            for cycle in &q.cycles {
                sqlx::query(
                    "INSERT INTO pot_bonus_weekly_cycles (account_id, matrix_id, user_id) \
                     VALUES ($1, $2, $3) \
                     ON CONFLICT (account_id, matrix_id) DO NOTHING",
                )
                .bind(cycle.account_id)
                .bind(cycle.matrix_id)
                .bind(q.user_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("Failed to insert cycle record: {e}"))?;
            }
        }

        tx.commit()
            .await
            .map_err(|e| format!("Failed to commit transaction: {e}"))?;

        Ok(())
    }
}
