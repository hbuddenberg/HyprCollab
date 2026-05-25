use std::sync::Arc;

use async_trait::async_trait;
use hyprcollab_core::errors::{CoreError, Result};
use hyprcollab_core::traits::Tool;
use hyprcollab_rag::RagPipeline;

// ── RagQueryTool ──────────────────────────────────────────────────────────────

pub struct RagQueryTool {
    pipeline: Arc<RagPipeline>,
}

impl RagQueryTool {
    pub fn new(pipeline: Arc<RagPipeline>) -> Self {
        Self { pipeline }
    }
}

#[async_trait]
impl Tool for RagQueryTool {
    fn name(&self) -> &str {
        "rag_query"
    }

    fn description(&self) -> &str {
        "Semantically search indexed documents and return the most relevant chunks."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Natural language search query" },
                "limit": { "type": "integer", "description": "Max results to return (default: 5)", "default": 5 }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> Result<String> {
        let query = args["query"]
            .as_str()
            .ok_or_else(|| CoreError::Tool("Missing 'query' parameter".into()))?;
        let limit = args["limit"].as_u64().unwrap_or(5) as usize;

        let results = self
            .pipeline
            .query(query, limit)
            .await
            .map_err(|e| CoreError::Tool(format!("rag_query: {e}")))?;

        let items: Vec<serde_json::Value> = results
            .iter()
            .map(|r| {
                serde_json::json!({
                    "content": r.chunk.content,
                    "source": r.chunk.metadata.source_file,
                    "score": r.score,
                })
            })
            .collect();

        serde_json::to_string(&items).map_err(|e| CoreError::Tool(e.to_string()))
    }
}

// ── RagIngestTool ─────────────────────────────────────────────────────────────

pub struct RagIngestTool {
    pipeline: Arc<RagPipeline>,
}

impl RagIngestTool {
    pub fn new(pipeline: Arc<RagPipeline>) -> Self {
        Self { pipeline }
    }
}

#[async_trait]
impl Tool for RagIngestTool {
    fn name(&self) -> &str {
        "rag_ingest"
    }

    fn description(&self) -> &str {
        "Ingest a file from disk into the RAG pipeline by path."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Absolute path to the file to ingest" }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> Result<String> {
        let path = args["path"]
            .as_str()
            .ok_or_else(|| CoreError::Tool("Missing 'path' parameter".into()))?;

        let data = tokio::fs::read(path)
            .await
            .map_err(|e| CoreError::Tool(format!("read file '{path}': {e}")))?;

        let filename = std::path::Path::new(path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        let result = self
            .pipeline
            .ingest(&data, filename)
            .await
            .map_err(|e| CoreError::Tool(format!("rag_ingest: {e}")))?;

        serde_json::to_string(&serde_json::json!({
            "chunks_created": result.chunks_created,
            "document_title": result.document_title,
            "source": filename,
        }))
        .map_err(|e| CoreError::Tool(e.to_string()))
    }
}

// ── RagSearchTool ─────────────────────────────────────────────────────────────

pub struct RagSearchTool {
    pipeline: Arc<RagPipeline>,
}

impl RagSearchTool {
    pub fn new(pipeline: Arc<RagPipeline>) -> Self {
        Self { pipeline }
    }
}

#[async_trait]
impl Tool for RagSearchTool {
    fn name(&self) -> &str {
        "rag_search"
    }

    fn description(&self) -> &str {
        "List all documents currently indexed in the RAG pipeline."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {}
        })
    }

    async fn execute(&self, _args: serde_json::Value) -> Result<String> {
        let sources = self
            .pipeline
            .list_sources()
            .await
            .map_err(|e| CoreError::Tool(format!("rag_search: {e}")))?;

        serde_json::to_string(&serde_json::json!({ "sources": sources }))
            .map_err(|e| CoreError::Tool(e.to_string()))
    }
}
