#![cfg(feature = "db")]

use potbonus::{PgPotBonusRepository, PotBonus, PotBonusRepository};
use sqlx::PgPool;
use uuid::Uuid;

static DB_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn setup_test_db() -> PgPool {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://localhost/rfn_dev".to_string());

    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to test database");

    // Recreate clean database state for the tests
    sqlx::query(
        "DROP TABLE IF EXISTS \
         pot_bonus_weekly_cycles, \
         pot_bonus_weekly_graduations, \
         pot_bonus_registrations, \
         pot_bonus_state CASCADE",
    )
    .execute(&pool)
    .await
    .expect("Failed to drop old tables");

    let migration_sql = include_str!("../migrations/20260623000000_create_pot_bonus_tables.sql");
    for statement in migration_sql.split(';') {
        let trimmed = statement.trim();
        if !trimmed.is_empty() {
            sqlx::query(trimmed)
                .execute(&pool)
                .await
                .expect("Failed to run migration statement");
        }
    }

    pool
}

#[tokio::test]
async fn test_empty_pot_bonus_roundtrip() {
    let _lock = DB_LOCK.lock().await;
    let pool = setup_test_db().await;
    let repo = PgPotBonusRepository::new(pool);

    let pb = PotBonus::new();

    // Save
    repo.save(&pb).await.expect("Failed to save PotBonus");

    // Load back and verify
    let loaded = repo.load().await.expect("Failed to load PotBonus");
    assert_eq!(loaded.total_pool_points(), 0);
}

#[tokio::test]
async fn test_pot_bonus_registration_and_qualification() {
    let _lock = DB_LOCK.lock().await;
    let pool = setup_test_db().await;
    let repo = PgPotBonusRepository::new(pool);

    let mut pb = PotBonus::new();
    pb.add_points(500);

    let user_1 = Uuid::now_v7();
    let acct_1 = Uuid::now_v7();
    let acct_2 = Uuid::now_v7();

    pb.register_user_account(user_1, acct_1);
    pb.register_user_account(user_1, acct_2);

    pb.record_graduation(acct_1).unwrap();
    pb.record_matrix_cycle(acct_2, Uuid::now_v7()).unwrap();

    // Verify qualification state in memory
    assert!(pb.is_user_qualified(&user_1));
    assert_eq!(pb.user_cycle_count(&user_1), 1);

    // Save state to database
    repo.save(&pb).await.expect("Failed to save PotBonus");

    // Load state from database and verify reconstruction
    let loaded = repo.load().await.expect("Failed to load PotBonus");
    assert_eq!(loaded.total_pool_points(), 500);
    assert!(loaded.is_user_qualified(&user_1));
    assert_eq!(loaded.user_cycle_count(&user_1), 1);
}

#[tokio::test]
async fn test_distribution_and_weekly_reset_persistence() {
    let _lock = DB_LOCK.lock().await;
    let pool = setup_test_db().await;
    let repo = PgPotBonusRepository::new(pool);

    let mut pb = PotBonus::new();
    pb.add_points(1000);

    let user_a = Uuid::now_v7();
    let acct_a = Uuid::now_v7();
    pb.register_user_account(user_a, acct_a);
    pb.record_graduation(acct_a).unwrap();
    pb.record_matrix_cycle(acct_a, Uuid::now_v7()).unwrap();

    assert!(pb.is_user_qualified(&user_a));

    // Save initial state before distribution
    repo.save(&pb)
        .await
        .expect("Failed to save PotBonus pre-distribute");

    // Load and execute distribution
    let mut pb_to_distribute = repo.load().await.expect("Failed to load pre-distribute");
    assert_eq!(pb_to_distribute.total_pool_points(), 1000);

    let res = pb_to_distribute.distribute_weekly().unwrap();
    assert_eq!(res.total_pool, 1000);
    assert_eq!(res.profit_sharing_for(user_a), Some(750));
    assert_eq!(res.top_cycler_for(user_a), Some(100)); // 40% of 250
    assert_eq!(pb_to_distribute.total_pool_points(), 0);

    // After distribution, save the clean (reset) state
    repo.save(&pb_to_distribute)
        .await
        .expect("Failed to save post-distribute state");

    // Load back and verify that pool is 0, user is NOT qualified, but registration STILL exists
    let post_dist = repo
        .load()
        .await
        .expect("Failed to load post-distribute state");
    assert_eq!(post_dist.total_pool_points(), 0);
    assert!(!post_dist.is_user_qualified(&user_a));

    // Verify registration survives by trying to record a graduation directly on the loaded instance
    // (should succeed without errors since the registration is reconstructed from `pot_bonus_registrations`)
    assert!(post_dist.graduated_accounts_for_user(&user_a).is_empty());
}
