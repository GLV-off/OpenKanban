use open_kanban_service::db;

#[tokio::test]
async fn applies_migrations_and_connection_pragmas() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let pool = db::open(db::path_options(dir.path().join("test.db")))
        .await
        .expect("test database");

    let journal_mode: String = sqlx::query_scalar("PRAGMA journal_mode")
        .fetch_one(&pool)
        .await
        .expect("journal mode");
    assert_eq!(journal_mode.to_ascii_lowercase(), "wal");

    let foreign_keys: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
        .fetch_one(&pool)
        .await
        .expect("foreign keys pragma");
    assert_eq!(foreign_keys, 1);

    let tables: Vec<String> =
        sqlx::query_scalar("SELECT name FROM sqlite_master WHERE type = 'table'")
            .fetch_all(&pool)
            .await
            .expect("table list");
    assert!(tables.iter().any(|name| name == "users"));
    assert!(tables.iter().any(|name| name == "sessions"));

    let applied: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
        .fetch_one(&pool)
        .await
        .expect("migrations table");
    assert!(applied >= 1);

    pool.close().await;
}

#[tokio::test]
async fn foreign_keys_reject_unknown_users() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let pool = db::open(db::path_options(dir.path().join("test.db")))
        .await
        .expect("test database");

    let result = sqlx::query(
        "INSERT INTO sessions (id, user_id, token_hash, issued_at, expires_at)
         VALUES ('s1', 'missing-user', 'hash', 'now', 'later')",
    )
    .execute(&pool)
    .await;

    assert!(result.is_err(), "foreign key must reject unknown user");

    pool.close().await;
}
