//! Prompt templates — CRUD with variable substitution using {{var}} syntax.

use std::collections::HashMap;

use hyprcollab_core::errors::{CoreError, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::store::MemoryStore;

// ── TemplateId ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TemplateId(pub Uuid);

impl TemplateId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for TemplateId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for TemplateId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ── PromptTemplate ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptTemplate {
    pub id: TemplateId,
    pub name: String,
    pub description: Option<String>,
    pub template: String,
    pub variables: Vec<String>,
    pub category: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

// ── TemplateEngine ────────────────────────────────────────────────────────────

/// CRUD + rendering for prompt templates, backed by the shared [`MemoryStore`].
pub struct TemplateEngine<'a> {
    store: &'a MemoryStore,
}

impl<'a> TemplateEngine<'a> {
    pub fn new(store: &'a MemoryStore) -> Self {
        Self { store }
    }

    // ── CRUD ──────────────────────────────────────────────────────────────

    /// Create a new template. Returns an error if `name` already exists.
    pub fn create_template(
        &self,
        name: &str,
        description: Option<String>,
        template: &str,
        variables: Vec<String>,
        category: Option<String>,
    ) -> Result<PromptTemplate> {
        let id = TemplateId::new();
        let now = chrono::Utc::now().to_rfc3339();
        let vars_json = serde_json::to_string(&variables)
            .map_err(|e| CoreError::Memory(format!("vars serialize: {e}")))?;

        let conn = self.store.lock_conn()?;
        conn.execute(
            "INSERT INTO prompt_templates
                (id, name, description, template, variables, category, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                id.to_string(),
                name,
                description,
                template,
                vars_json,
                category,
                now,
                now,
            ],
        )
        .map_err(|e| CoreError::Memory(format!("create_template: {e}")))?;

        Ok(PromptTemplate {
            id,
            name: name.to_string(),
            description,
            template: template.to_string(),
            variables,
            category,
            created_at: now.clone(),
            updated_at: now,
        })
    }

    /// Get a template by ID.
    pub fn get_template(&self, id: TemplateId) -> Result<Option<PromptTemplate>> {
        let conn = self.store.lock_conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT id, name, description, template, variables, category, created_at, updated_at
                 FROM prompt_templates WHERE id = ?1",
            )
            .map_err(|e| CoreError::Memory(format!("get_template prepare: {e}")))?;

        let mut rows = stmt
            .query(rusqlite::params![id.to_string()])
            .map_err(|e| CoreError::Memory(format!("get_template query: {e}")))?;

        match rows.next().map_err(|e| CoreError::Memory(format!("get_template next: {e}")))? {
            Some(row) => Ok(Some(
                template_from_row(row)
                    .map_err(|e| CoreError::Memory(format!("template row: {e}")))?,
            )),
            None => Ok(None),
        }
    }

    /// Get a template by name.
    pub fn get_template_by_name(&self, name: &str) -> Result<Option<PromptTemplate>> {
        let conn = self.store.lock_conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT id, name, description, template, variables, category, created_at, updated_at
                 FROM prompt_templates WHERE name = ?1",
            )
            .map_err(|e| CoreError::Memory(format!("get_template_by_name prepare: {e}")))?;

        let mut rows = stmt
            .query(rusqlite::params![name])
            .map_err(|e| CoreError::Memory(format!("get_template_by_name query: {e}")))?;

        match rows
            .next()
            .map_err(|e| CoreError::Memory(format!("get_template_by_name next: {e}")))?
        {
            Some(row) => Ok(Some(
                template_from_row(row)
                    .map_err(|e| CoreError::Memory(format!("template row: {e}")))?,
            )),
            None => Ok(None),
        }
    }

    /// List all templates, ordered by name.
    pub fn list_templates(&self) -> Result<Vec<PromptTemplate>> {
        let conn = self.store.lock_conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT id, name, description, template, variables, category, created_at, updated_at
                 FROM prompt_templates ORDER BY name ASC",
            )
            .map_err(|e| CoreError::Memory(format!("list_templates prepare: {e}")))?;

        let rows = stmt
            .query_map([], template_from_row)
            .map_err(|e| CoreError::Memory(format!("list_templates query: {e}")))?;

        let mut templates = Vec::new();
        for row in rows {
            templates
                .push(row.map_err(|e| CoreError::Memory(format!("list_templates row: {e}")))?);
        }
        Ok(templates)
    }

    /// Update an existing template. Returns `true` if found and updated.
    pub fn update_template(
        &self,
        id: TemplateId,
        name: &str,
        description: Option<String>,
        template: &str,
        variables: Vec<String>,
        category: Option<String>,
    ) -> Result<bool> {
        let now = chrono::Utc::now().to_rfc3339();
        let vars_json = serde_json::to_string(&variables)
            .map_err(|e| CoreError::Memory(format!("vars serialize: {e}")))?;

        let conn = self.store.lock_conn()?;
        let rows = conn
            .execute(
                "UPDATE prompt_templates
                 SET name = ?1, description = ?2, template = ?3,
                     variables = ?4, category = ?5, updated_at = ?6
                 WHERE id = ?7",
                rusqlite::params![
                    name,
                    description,
                    template,
                    vars_json,
                    category,
                    now,
                    id.to_string(),
                ],
            )
            .map_err(|e| CoreError::Memory(format!("update_template: {e}")))?;

        Ok(rows > 0)
    }

    /// Delete a template by ID. Returns `true` if found and deleted.
    pub fn delete_template(&self, id: TemplateId) -> Result<bool> {
        let conn = self.store.lock_conn()?;
        let rows = conn
            .execute(
                "DELETE FROM prompt_templates WHERE id = ?1",
                rusqlite::params![id.to_string()],
            )
            .map_err(|e| CoreError::Memory(format!("delete_template: {e}")))?;

        Ok(rows > 0)
    }

    // ── Rendering ─────────────────────────────────────────────────────────

    /// Render a template by substituting `{{variable}}` placeholders.
    /// Returns an error if any declared variable is missing from `vars`.
    pub fn render_template(
        &self,
        id: TemplateId,
        vars: &HashMap<String, String>,
    ) -> Result<String> {
        let tmpl = self
            .get_template(id)?
            .ok_or_else(|| CoreError::Memory(format!("template '{id}' not found")))?;

        render_template_str(&tmpl.template, &tmpl.variables, vars)
    }

    // ── Built-ins ─────────────────────────────────────────────────────────

    /// Insert the four built-in templates if they do not already exist.
    pub fn load_builtin_templates(&self) -> Result<()> {
        let builtins: &[(&str, &str, &str, &[&str], &str)] = &[
            (
                "code-review",
                "Review code for bugs, style issues, and improvements",
                "Please review the following {{language}} code for bugs, style issues, and improvements:\n\n```{{language}}\n{{code}}\n```",
                &["language", "code"],
                "coding",
            ),
            (
                "explain",
                "Explain a topic at a specified detail level",
                "Please explain {{topic}} at {{detail_level}} detail level.",
                &["topic", "detail_level"],
                "education",
            ),
            (
                "summarize",
                "Summarize a piece of content",
                "Please summarize the following {{content_type}}:\n\n{{content}}",
                &["content_type", "content"],
                "general",
            ),
            (
                "translate",
                "Translate content between languages",
                "Please translate the following text from {{source_lang}} to {{target_lang}}:\n\n{{content}}",
                &["source_lang", "target_lang", "content"],
                "language",
            ),
        ];

        let conn = self.store.lock_conn()?;
        let now = chrono::Utc::now().to_rfc3339();

        for (name, description, template, vars, category) in builtins {
            let id = Uuid::new_v4().to_string();
            let vars_json = serde_json::to_string(vars)
                .map_err(|e| CoreError::Memory(format!("vars serialize: {e}")))?;

            conn.execute(
                "INSERT OR IGNORE INTO prompt_templates
                    (id, name, description, template, variables, category, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                rusqlite::params![id, name, description, template, vars_json, category, now, now],
            )
            .map_err(|e| CoreError::Memory(format!("load builtin template '{name}': {e}")))?;
        }

        Ok(())
    }
}

// ── Row helper ────────────────────────────────────────────────────────────────

fn template_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<PromptTemplate> {
    let id_str: String = row.get(0)?;
    let name: String = row.get(1)?;
    let description: Option<String> = row.get(2).unwrap_or(None);
    let template: String = row.get(3)?;
    let vars_json: Option<String> = row.get(4).unwrap_or(None);
    let category: Option<String> = row.get(5).unwrap_or(None);
    let created_at: String = row.get(6)?;
    let updated_at: String = row.get(7)?;

    let id = Uuid::parse_str(&id_str).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::from(e))
    })?;

    let variables: Vec<String> = vars_json
        .as_deref()
        .map(serde_json::from_str)
        .transpose()
        .map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::from(e))
        })?
        .unwrap_or_default();

    Ok(PromptTemplate {
        id: TemplateId(id),
        name,
        description,
        template,
        variables,
        category,
        created_at,
        updated_at,
    })
}

// ── Template rendering ────────────────────────────────────────────────────────

/// Substitute `{{variable}}` placeholders. All declared variables must be present.
pub fn render_template_str(
    template: &str,
    required_vars: &[String],
    vars: &HashMap<String, String>,
) -> Result<String> {
    for var in required_vars {
        if !vars.contains_key(var) {
            return Err(CoreError::Memory(format!(
                "missing required variable: {var}"
            )));
        }
    }

    let mut result = template.to_string();
    for (key, value) in vars {
        let placeholder = format!("{{{{{}}}}}", key);
        result = result.replace(&placeholder, value);
    }
    Ok(result)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn open_store() -> MemoryStore {
        MemoryStore::open_in_memory().unwrap()
    }

    fn vars(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn create_and_get_template() {
        let store = open_store();
        let engine = TemplateEngine::new(&store);

        let tmpl = engine
            .create_template(
                "test",
                Some("A test template".into()),
                "Hello {{name}}!",
                vec!["name".into()],
                Some("general".into()),
            )
            .unwrap();

        assert_eq!(tmpl.name, "test");
        assert_eq!(tmpl.variables, vec!["name"]);

        let fetched = engine.get_template(tmpl.id).unwrap().unwrap();
        assert_eq!(fetched.id, tmpl.id);
        assert_eq!(fetched.template, "Hello {{name}}!");
    }

    #[test]
    fn get_template_not_found() {
        let store = open_store();
        let engine = TemplateEngine::new(&store);
        assert!(engine.get_template(TemplateId::new()).unwrap().is_none());
    }

    #[test]
    fn get_template_by_name() {
        let store = open_store();
        let engine = TemplateEngine::new(&store);
        engine
            .create_template("named-tmpl", None, "content {{x}}", vec!["x".into()], None)
            .unwrap();

        let found = engine.get_template_by_name("named-tmpl").unwrap();
        assert!(found.is_some());
        let missing = engine.get_template_by_name("no-such").unwrap();
        assert!(missing.is_none());
    }

    #[test]
    fn list_templates_returns_all() {
        let store = open_store();
        let engine = TemplateEngine::new(&store);
        engine.create_template("a", None, "{{x}}", vec!["x".into()], None).unwrap();
        engine.create_template("b", None, "{{y}}", vec!["y".into()], None).unwrap();

        let list = engine.list_templates().unwrap();
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn update_template() {
        let store = open_store();
        let engine = TemplateEngine::new(&store);
        let tmpl = engine
            .create_template("orig", None, "{{a}}", vec!["a".into()], None)
            .unwrap();

        let updated =
            engine.update_template(tmpl.id, "orig", None, "{{a}} {{b}}", vec!["a".into(), "b".into()], None).unwrap();
        assert!(updated);

        let fetched = engine.get_template(tmpl.id).unwrap().unwrap();
        assert_eq!(fetched.template, "{{a}} {{b}}");
        assert_eq!(fetched.variables.len(), 2);
    }

    #[test]
    fn delete_template() {
        let store = open_store();
        let engine = TemplateEngine::new(&store);
        let tmpl =
            engine.create_template("del", None, "{{x}}", vec!["x".into()], None).unwrap();

        assert!(engine.delete_template(tmpl.id).unwrap());
        assert!(!engine.delete_template(tmpl.id).unwrap());
        assert!(engine.get_template(tmpl.id).unwrap().is_none());
    }

    #[test]
    fn render_template_substitutes_variables() {
        let store = open_store();
        let engine = TemplateEngine::new(&store);
        let tmpl = engine
            .create_template("greet", None, "Hello {{name}}, you are {{age}}!", vec!["name".into(), "age".into()], None)
            .unwrap();

        let rendered =
            engine.render_template(tmpl.id, &vars(&[("name", "Alice"), ("age", "30")])).unwrap();
        assert_eq!(rendered, "Hello Alice, you are 30!");
    }

    #[test]
    fn render_template_missing_var_returns_error() {
        let store = open_store();
        let engine = TemplateEngine::new(&store);
        let tmpl =
            engine.create_template("err", None, "{{a}} {{b}}", vec!["a".into(), "b".into()], None).unwrap();

        let result = engine.render_template(tmpl.id, &vars(&[("a", "only_a")]));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("missing required variable: b"));
    }

    #[test]
    fn load_builtin_templates_creates_four() {
        let store = open_store();
        let engine = TemplateEngine::new(&store);
        engine.load_builtin_templates().unwrap();

        let templates = engine.list_templates().unwrap();
        assert_eq!(templates.len(), 4);
        let names: Vec<&str> = templates.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"code-review"));
        assert!(names.contains(&"explain"));
        assert!(names.contains(&"summarize"));
        assert!(names.contains(&"translate"));
    }

    #[test]
    fn load_builtin_templates_idempotent() {
        let store = open_store();
        let engine = TemplateEngine::new(&store);
        engine.load_builtin_templates().unwrap();
        engine.load_builtin_templates().unwrap();

        let templates = engine.list_templates().unwrap();
        assert_eq!(templates.len(), 4);
    }

    #[test]
    fn render_builtin_code_review() {
        let store = open_store();
        let engine = TemplateEngine::new(&store);
        engine.load_builtin_templates().unwrap();

        let tmpl = engine.get_template_by_name("code-review").unwrap().unwrap();
        let rendered = engine
            .render_template(tmpl.id, &vars(&[("language", "Rust"), ("code", "fn main() {}")]))
            .unwrap();

        assert!(rendered.contains("Rust"));
        assert!(rendered.contains("fn main() {}"));
    }
}
