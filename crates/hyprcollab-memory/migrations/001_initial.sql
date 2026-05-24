-- Migration 001: Initial schema
-- Creates the core tables for the HyprCollab memory engine.

CREATE TABLE IF NOT EXISTS chats (
    id            TEXT PRIMARY KEY,
    title         TEXT,
    workspace_id  TEXT,
    persona_id    TEXT,
    agent_role_id TEXT,
    model         TEXT,
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS messages (
    id         TEXT PRIMARY KEY,
    chat_id    TEXT NOT NULL REFERENCES chats(id) ON DELETE CASCADE,
    role       TEXT NOT NULL,
    content    TEXT,
    tool_calls TEXT,   -- JSON array of ToolCall objects
    artifacts  TEXT,   -- JSON array of Artifact objects
    timestamp  TEXT NOT NULL,
    metadata   TEXT    -- arbitrary JSON value
);

CREATE TABLE IF NOT EXISTS chat_settings (
    chat_id TEXT NOT NULL REFERENCES chats(id) ON DELETE CASCADE,
    key     TEXT NOT NULL,
    value   TEXT,
    PRIMARY KEY (chat_id, key)
);

CREATE TABLE IF NOT EXISTS folder_configs (
    path        TEXT PRIMARY KEY,
    config_json TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS approval_rules (
    id         TEXT PRIMARY KEY,
    pattern    TEXT NOT NULL,
    action     TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS registered_agents (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    config_path TEXT,
    enabled     BOOLEAN NOT NULL DEFAULT 1
);

-- Indexes for common query patterns
CREATE INDEX IF NOT EXISTS idx_messages_chat_id ON messages(chat_id);
CREATE INDEX IF NOT EXISTS idx_messages_timestamp ON messages(timestamp);
