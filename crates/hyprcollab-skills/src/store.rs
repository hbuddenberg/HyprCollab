//! SQLite-backed skill store sharing the MemoryStore connection.

use std::sync::{Arc, Mutex};

use rusqlite::{params, Connection};

use crate::{Skill, SkillCreateRequest, SkillId, SkillUpdateRequest};

// ── Error ─────────────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("skill not found: {0}")]
    NotFound(SkillId),
}

pub type Result<T> = std::result::Result<T, StoreError>;

// ── SkillStore ────────────────────────────────────────────────────────────────

/// CRUD store for skills backed by the shared SQLite connection.
pub struct SkillStore {
    conn: Arc<Mutex<Connection>>,
}

impl SkillStore {
    /// Create a new store from an existing connection arc.
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    // ── Write ─────────────────────────────────────────────────────────────

    /// Insert a new skill.
    pub fn add_skill(&self, req: &SkillCreateRequest) -> Result<Skill> {
        let now = chrono::Utc::now().to_rfc3339();
        let skill = Skill {
            id: SkillId::new(),
            name: req.name.clone(),
            description: req.description.clone(),
            category: req.category.clone(),
            trigger_patterns: req.trigger_patterns.clone(),
            instructions: req.instructions.clone(),
            enabled: req.enabled.unwrap_or(true),
            priority: req.priority.unwrap_or(0),
            usage_count: 0,
            success_rate: 0.0,
            created_at: now.clone(),
            updated_at: now,
        };
        self.insert_skill(&skill)?;
        Ok(skill)
    }

    fn insert_skill(&self, skill: &Skill) -> Result<()> {
        let patterns_json = serde_json::to_string(&skill.trigger_patterns)?;
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        conn.execute(
            "INSERT INTO skills
             (id, name, description, category, trigger_patterns, instructions,
              enabled, priority, usage_count, success_rate, created_at, updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
            params![
                skill.id.to_string(),
                skill.name,
                skill.description,
                skill.category,
                patterns_json,
                skill.instructions,
                skill.enabled as i64,
                skill.priority,
                skill.usage_count,
                skill.success_rate,
                skill.created_at,
                skill.updated_at,
            ],
        )?;
        Ok(())
    }

    /// Apply a partial update to an existing skill.
    pub fn update_skill(&self, id: SkillId, req: &SkillUpdateRequest) -> Result<Skill> {
        let mut skill = self.get_skill(id)?;
        if let Some(d) = &req.description {
            skill.description = d.clone();
        }
        if let Some(c) = &req.category {
            skill.category = Some(c.clone());
        }
        if let Some(p) = &req.trigger_patterns {
            skill.trigger_patterns = p.clone();
        }
        if let Some(i) = &req.instructions {
            skill.instructions = i.clone();
        }
        if let Some(e) = req.enabled {
            skill.enabled = e;
        }
        if let Some(p) = req.priority {
            skill.priority = p;
        }
        skill.updated_at = chrono::Utc::now().to_rfc3339();

        let patterns_json = serde_json::to_string(&skill.trigger_patterns)?;
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let rows = conn.execute(
            "UPDATE skills
             SET description=?1, category=?2, trigger_patterns=?3, instructions=?4,
                 enabled=?5, priority=?6, updated_at=?7
             WHERE id=?8",
            params![
                skill.description,
                skill.category,
                patterns_json,
                skill.instructions,
                skill.enabled as i64,
                skill.priority,
                skill.updated_at,
                id.to_string(),
            ],
        )?;
        if rows == 0 {
            return Err(StoreError::NotFound(id));
        }
        Ok(skill)
    }

    /// Delete a skill by ID.
    pub fn delete_skill(&self, id: SkillId) -> Result<()> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let rows = conn.execute("DELETE FROM skills WHERE id=?1", params![id.to_string()])?;
        if rows == 0 {
            return Err(StoreError::NotFound(id));
        }
        Ok(())
    }

    /// Increment usage_count and update success_rate for a skill.
    pub fn record_usage(&self, id: SkillId, success: bool) -> Result<()> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        // success_rate = (success_rate * usage_count + new_outcome) / (usage_count + 1)
        let outcome: f64 = if success { 1.0 } else { 0.0 };
        let rows = conn.execute(
            "UPDATE skills
             SET success_rate = (success_rate * usage_count + ?1) / (usage_count + 1),
                 usage_count  = usage_count + 1,
                 updated_at   = ?2
             WHERE id = ?3",
            params![outcome, chrono::Utc::now().to_rfc3339(), id.to_string()],
        )?;
        if rows == 0 {
            return Err(StoreError::NotFound(id));
        }
        Ok(())
    }

    // ── Read ──────────────────────────────────────────────────────────────

    /// Fetch a skill by ID.
    pub fn get_skill(&self, id: SkillId) -> Result<Skill> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let result = conn.query_row(
            "SELECT id,name,description,category,trigger_patterns,instructions,
                    enabled,priority,usage_count,success_rate,created_at,updated_at
             FROM skills WHERE id=?1",
            params![id.to_string()],
            row_to_skill,
        );
        match result {
            Ok(s) => Ok(s),
            Err(rusqlite::Error::QueryReturnedNoRows) => Err(StoreError::NotFound(id)),
            Err(e) => Err(StoreError::Sqlite(e)),
        }
    }

    /// List all skills, ordered by priority DESC then usage_count DESC.
    pub fn list_skills(&self) -> Result<Vec<Skill>> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let mut stmt = conn.prepare(
            "SELECT id,name,description,category,trigger_patterns,instructions,
                    enabled,priority,usage_count,success_rate,created_at,updated_at
             FROM skills ORDER BY priority DESC, usage_count DESC",
        )?;
        let rows = stmt.query_map([], row_to_skill)?;
        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(StoreError::Sqlite)
    }

    // ── Built-in skills ───────────────────────────────────────────────────

    /// Insert default built-in skills if they don't already exist.
    pub fn load_builtin_skills(&self) -> Result<()> {
        let builtins = builtin_skills();
        for skill in &builtins {
            let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
            let exists: bool = conn.query_row(
                "SELECT COUNT(*) FROM skills WHERE name=?1",
                params![skill.name],
                |r| r.get::<_, i64>(0),
            )? > 0;
            drop(conn);
            if !exists {
                self.insert_skill(skill)?;
            }
        }
        Ok(())
    }
}

// ── Row mapping ───────────────────────────────────────────────────────────────

fn row_to_skill(row: &rusqlite::Row<'_>) -> rusqlite::Result<Skill> {
    let id_str: String = row.get(0)?;
    let patterns_json: String = row.get(4)?;
    let enabled_int: i64 = row.get(6)?;

    let id = uuid::Uuid::parse_str(&id_str)
        .map(SkillId)
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))?;

    let trigger_patterns: Vec<String> =
        serde_json::from_str(&patterns_json)
            .map_err(|e| rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(e)))?;

    Ok(Skill {
        id,
        name: row.get(1)?,
        description: row.get(2)?,
        category: row.get(3)?,
        trigger_patterns,
        instructions: row.get(5)?,
        enabled: enabled_int != 0,
        priority: row.get(7)?,
        usage_count: row.get(8)?,
        success_rate: row.get(9)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
    })
}

// ── Built-in skills ───────────────────────────────────────────────────────────

fn builtin_skills() -> Vec<Skill> {
    let now = chrono::Utc::now().to_rfc3339();
    vec![
        Skill {
            id: SkillId::new(),
            name: "code-review".into(),
            description: "Expert code reviewer for any programming language.".into(),
            category: Some("development".into()),
            trigger_patterns: vec![
                r"(?i)review".into(),
                r"(?i)code review".into(),
                r"(?i)pull request".into(),
                r"(?i)\bPR\b".into(),
            ],
            instructions: "You are an expert code reviewer. \
                Analyse the submitted code for correctness, performance, security, and style. \
                Structure your review as: Summary, Issues (Critical / Major / Minor), \
                and Suggestions. Be concise and actionable.".into(),
            enabled: true,
            priority: 10,
            usage_count: 0,
            success_rate: 0.0,
            created_at: now.clone(),
            updated_at: now.clone(),
        },
        Skill {
            id: SkillId::new(),
            name: "debugging".into(),
            description: "Systematic debugging assistant.".into(),
            category: Some("development".into()),
            trigger_patterns: vec![
                r"(?i)debug".into(),
                r"(?i)bug".into(),
                r"(?i)error".into(),
                r"(?i)exception".into(),
                r"(?i)crash".into(),
            ],
            instructions: "You are an expert debugger. \
                When given an error or unexpected behaviour, follow this process: \
                1. Restate the observed symptoms. \
                2. Form hypotheses about root causes. \
                3. Suggest targeted diagnostic steps. \
                4. Propose the most likely fix with an explanation.".into(),
            enabled: true,
            priority: 8,
            usage_count: 0,
            success_rate: 0.0,
            created_at: now.clone(),
            updated_at: now.clone(),
        },
        Skill {
            id: SkillId::new(),
            name: "documentation".into(),
            description: "Generates clear technical documentation.".into(),
            category: Some("writing".into()),
            trigger_patterns: vec![
                r"(?i)document".into(),
                r"(?i)README".into(),
                r"(?i)docs?".into(),
                r"(?i)write.*docs?".into(),
            ],
            instructions: "You are a technical writer. \
                Produce clear, accurate documentation suitable for the stated audience. \
                Use Markdown unless another format is requested. \
                Include: Overview, Prerequisites, Usage examples, \
                and a Troubleshooting section where relevant.".into(),
            enabled: true,
            priority: 5,
            usage_count: 0,
            success_rate: 0.0,
            created_at: now.clone(),
            updated_at: now,
        },
    ]
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn open_store() -> SkillStore {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE skills (
                id               TEXT PRIMARY KEY,
                name             TEXT NOT NULL UNIQUE,
                description      TEXT NOT NULL,
                category         TEXT,
                trigger_patterns TEXT NOT NULL,
                instructions     TEXT NOT NULL,
                enabled          INTEGER NOT NULL DEFAULT 1,
                priority         INTEGER NOT NULL DEFAULT 0,
                usage_count      INTEGER NOT NULL DEFAULT 0,
                success_rate     REAL    NOT NULL DEFAULT 0.0,
                created_at       TEXT NOT NULL,
                updated_at       TEXT NOT NULL
            );",
        )
        .unwrap();
        SkillStore::new(Arc::new(Mutex::new(conn)))
    }

    fn req(name: &str) -> SkillCreateRequest {
        SkillCreateRequest {
            name: name.into(),
            description: "A test skill.".into(),
            category: None,
            trigger_patterns: vec!["test".into()],
            instructions: "Do the thing.".into(),
            enabled: None,
            priority: None,
        }
    }

    #[test]
    fn add_and_get_skill() {
        let store = open_store();
        let skill = store.add_skill(&req("my-skill")).unwrap();
        let fetched = store.get_skill(skill.id).unwrap();
        assert_eq!(fetched.name, "my-skill");
    }

    #[test]
    fn list_skills_returns_all() {
        let store = open_store();
        store.add_skill(&req("alpha")).unwrap();
        store.add_skill(&req("beta")).unwrap();
        let list = store.list_skills().unwrap();
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn update_skill_changes_description() {
        let store = open_store();
        let skill = store.add_skill(&req("updatable")).unwrap();
        let upd = SkillUpdateRequest {
            description: Some("Updated description.".into()),
            category: None,
            trigger_patterns: None,
            instructions: None,
            enabled: None,
            priority: None,
        };
        let updated = store.update_skill(skill.id, &upd).unwrap();
        assert_eq!(updated.description, "Updated description.");
    }

    #[test]
    fn delete_skill_removes_it() {
        let store = open_store();
        let skill = store.add_skill(&req("deletable")).unwrap();
        store.delete_skill(skill.id).unwrap();
        assert!(matches!(store.get_skill(skill.id), Err(StoreError::NotFound(_))));
    }

    #[test]
    fn get_unknown_skill_returns_not_found() {
        let store = open_store();
        assert!(matches!(store.get_skill(SkillId::new()), Err(StoreError::NotFound(_))));
    }

    #[test]
    fn record_usage_updates_rate() {
        let store = open_store();
        let skill = store.add_skill(&req("tracked")).unwrap();
        store.record_usage(skill.id, true).unwrap();
        store.record_usage(skill.id, false).unwrap();
        let s = store.get_skill(skill.id).unwrap();
        assert_eq!(s.usage_count, 2);
        assert!((s.success_rate - 0.5).abs() < 1e-9);
    }

    #[test]
    fn load_builtin_skills_inserts_three() {
        let store = open_store();
        store.load_builtin_skills().unwrap();
        let list = store.list_skills().unwrap();
        assert_eq!(list.len(), 3);
    }

    #[test]
    fn load_builtin_skills_idempotent() {
        let store = open_store();
        store.load_builtin_skills().unwrap();
        store.load_builtin_skills().unwrap();
        let list = store.list_skills().unwrap();
        assert_eq!(list.len(), 3);
    }
}
