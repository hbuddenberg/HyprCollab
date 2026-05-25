-- Migration 003: Skills engine

CREATE TABLE IF NOT EXISTS skills (
    id               TEXT PRIMARY KEY,
    name             TEXT NOT NULL UNIQUE,
    description      TEXT NOT NULL,
    category         TEXT,
    trigger_patterns TEXT NOT NULL,   -- JSON array of regex strings
    instructions     TEXT NOT NULL,
    enabled          INTEGER NOT NULL DEFAULT 1,
    priority         INTEGER NOT NULL DEFAULT 0,
    usage_count      INTEGER NOT NULL DEFAULT 0,
    success_rate     REAL    NOT NULL DEFAULT 0.0,
    created_at       TEXT NOT NULL,
    updated_at       TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_skills_name     ON skills(name);
CREATE INDEX IF NOT EXISTS idx_skills_enabled  ON skills(enabled);
CREATE INDEX IF NOT EXISTS idx_skills_priority ON skills(priority);
