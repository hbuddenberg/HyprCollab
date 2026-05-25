//! Database schema migrations.
//!
//! Uses a `schema_version` meta-table to track which migrations have been applied.
//! Each migration is wrapped in its own transaction.

use hyprcollab_core::errors::{CoreError, Result};

/// Run all pending migrations in order.
pub fn run(conn: &rusqlite::Connection) -> Result<()> {
    // Ensure the schema_version tracking table exists.
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_version (
            version INTEGER PRIMARY KEY,
            name    TEXT NOT NULL,
            applied TEXT NOT NULL
        );",
    )
    .map_err(|e| CoreError::Memory(format!("failed to create schema_version table: {e}")))?;

    let current: i32 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_version",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let migrations: &[(i32, &str, &str)] = &[
        (1, "001_initial", include_str!("../migrations/001_initial.sql")),
        (2, "002_working_memory", include_str!("../migrations/002_working_memory.sql")),
    ];

    for &(version, name, sql) in migrations {
        if version > current {
            let tx = conn
                .unchecked_transaction()
                .map_err(|e| CoreError::Memory(format!("failed to begin transaction: {e}")))?;

            tx.execute_batch(sql)
                .map_err(|e| CoreError::Memory(format!("migration {name} failed: {e}")))?;

            let now = chrono::Utc::now().to_rfc3339();
            tx.execute(
                "INSERT INTO schema_version (version, name, applied) VALUES (?1, ?2, ?3)",
                rusqlite::params![version, name, now],
            )
            .map_err(|e| CoreError::Memory(format!("failed to record migration {name}: {e}")))?;

            tx.commit().map_err(|e| {
                CoreError::Memory(format!("failed to commit migration {name}: {e}"))
            })?;

            tracing::info!(version, name, "applied migration");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_run_on_fresh_db() {
        let conn = rusqlite::Connection::open_in_memory().expect("open in-memory db");
        run(&conn).expect("migrations should succeed");

        // Verify tables were created.
        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap();
        let tables: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();

        assert!(tables.contains(&"chats".to_string()));
        assert!(tables.contains(&"messages".to_string()));
        assert!(tables.contains(&"chat_settings".to_string()));
        assert!(tables.contains(&"folder_configs".to_string()));
        assert!(tables.contains(&"approval_rules".to_string()));
        assert!(tables.contains(&"registered_agents".to_string()));
        assert!(tables.contains(&"facts".to_string()));
    }

    #[test]
    fn migrations_are_idempotent() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        run(&conn).expect("first run");
        run(&conn).expect("second run should be no-op");

        let version: i32 = conn
            .query_row("SELECT MAX(version) FROM schema_version", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(version, 2);
    }
}
