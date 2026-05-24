//! Memory tools — store and search facts in agent memory.

use async_trait::async_trait;
use hyprcollab_core::errors::{CoreError, Result};
use hyprcollab_core::traits::Tool;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

/// In-memory fact store (will be replaced with SQLite/LanceDB later).
pub struct FactStore {
    facts: Mutex<Vec<Fact>>,
}

/// A single fact stored in memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fact {
    pub id: String,
    pub category: FactCategory,
    pub content: String,
    pub confidence: f32,
    pub created_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FactCategory {
    Preference,
    Fact,
    Pattern,
    Correction,
}

/// Memory tools: store and retrieve facts.
pub struct MemoryTools {
    store: std::sync::Arc<FactStore>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
enum MemoryParams {
    Store {
        category: FactCategory,
        content: String,
        #[serde(default = "default_confidence")]
        confidence: f32,
    },
    Search {
        query: String,
        #[serde(default = "default_limit")]
        limit: usize,
    },
    List {
        #[serde(default)]
        category: Option<FactCategory>,
    },
    Delete {
        id: String,
    },
}

fn default_confidence() -> f32 {
    1.0
}

fn default_limit() -> usize {
    10
}

impl Default for FactStore {
    fn default() -> Self {
        Self::new()
    }
}

impl FactStore {
    pub fn new() -> Self {
        Self {
            facts: Mutex::new(Vec::new()),
        }
    }

    pub fn store(&self, category: FactCategory, content: String, confidence: f32) -> Fact {
        let mut facts = self.facts.lock().unwrap();
        let id = format!("fact-{}", facts.len() + 1);
        let fact = Fact {
            id: id.clone(),
            category,
            content,
            confidence,
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        facts.push(fact.clone());
        fact
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<Fact> {
        let facts = self.facts.lock().unwrap();
        let query_lower = query.to_lowercase();
        let mut results: Vec<Fact> = facts
            .iter()
            .filter(|f| f.content.to_lowercase().contains(&query_lower))
            .cloned()
            .collect();
        results.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
        results.truncate(limit);
        results
    }

    pub fn list(&self, category: Option<FactCategory>) -> Vec<Fact> {
        let facts = self.facts.lock().unwrap();
        facts
            .iter()
            .filter(|f| category.is_none() || Some(f.category) == category)
            .cloned()
            .collect()
    }

    pub fn delete(&self, id: &str) -> bool {
        let mut facts = self.facts.lock().unwrap();
        let before = facts.len();
        facts.retain(|f| f.id != id);
        facts.len() < before
    }
}

impl MemoryTools {
    pub fn new() -> Self {
        Self {
            store: std::sync::Arc::new(FactStore::new()),
        }
    }

    pub fn with_store(store: std::sync::Arc<FactStore>) -> Self {
        Self { store }
    }

    pub fn store(&self) -> &std::sync::Arc<FactStore> {
        &self.store
    }
}

impl Default for MemoryTools {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for MemoryTools {
    fn name(&self) -> &str {
        "memory"
    }

    fn description(&self) -> &str {
        "Store, search, list, or delete facts in agent memory. Facts persist across conversations."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["store", "search", "list", "delete"],
                    "description": "The memory operation to perform"
                },
                "category": {
                    "type": "string",
                    "enum": ["preference", "fact", "pattern", "correction"],
                    "description": "Fact category (for store/list)"
                },
                "content": {
                    "type": "string",
                    "description": "Fact content (for store)"
                },
                "query": {
                    "type": "string",
                    "description": "Search query (for search)"
                },
                "id": {
                    "type": "string",
                    "description": "Fact ID (for delete)"
                },
                "confidence": {
                    "type": "number",
                    "description": "Confidence score 0.0-1.0 (for store, default: 1.0)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Max results (for search, default: 10)"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> Result<String> {
        let params: MemoryParams = serde_json::from_value(args)
            .map_err(|e| CoreError::Tool(format!("Invalid memory params: {e}")))?;

        match params {
            MemoryParams::Store {
                category,
                content,
                confidence,
            } => {
                let fact = self.store.store(category, content, confidence);
                Ok(serde_json::to_string(&fact)
                    .unwrap_or_else(|_| "Stored fact".to_string()))
            }
            MemoryParams::Search { query, limit } => {
                let results = self.store.search(&query, limit);
                Ok(serde_json::to_string(&results)
                    .unwrap_or_else(|_| "[]".to_string()))
            }
            MemoryParams::List { category } => {
                let facts = self.store.list(category);
                Ok(serde_json::to_string(&facts)
                    .unwrap_or_else(|_| "[]".to_string()))
            }
            MemoryParams::Delete { id } => {
                let deleted = self.store.delete(&id);
                Ok(serde_json::json!({"deleted": deleted, "id": id}).to_string())
            }
        }
    }
}
