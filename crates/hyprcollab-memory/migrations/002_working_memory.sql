-- Migration 002: Working memory — facts table with FTS5 full-text search

CREATE TABLE IF NOT EXISTS facts (
    id           TEXT PRIMARY KEY,
    category     TEXT NOT NULL,
    content      TEXT NOT NULL,
    confidence   REAL NOT NULL DEFAULT 0.8,
    source       TEXT,
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL,
    access_count INTEGER NOT NULL DEFAULT 0
);

CREATE VIRTUAL TABLE IF NOT EXISTS facts_fts USING fts5(
    content,
    category,
    content=facts,
    content_rowid=rowid
);

-- Keep FTS5 index in sync with the facts table
CREATE TRIGGER IF NOT EXISTS facts_ai AFTER INSERT ON facts BEGIN
    INSERT INTO facts_fts(rowid, content, category)
    VALUES (new.rowid, new.content, new.category);
END;

CREATE TRIGGER IF NOT EXISTS facts_ad AFTER DELETE ON facts BEGIN
    INSERT INTO facts_fts(facts_fts, rowid, content, category)
    VALUES ('delete', old.rowid, old.content, old.category);
END;

CREATE TRIGGER IF NOT EXISTS facts_au AFTER UPDATE ON facts BEGIN
    INSERT INTO facts_fts(facts_fts, rowid, content, category)
    VALUES ('delete', old.rowid, old.content, old.category);
    INSERT INTO facts_fts(rowid, content, category)
    VALUES (new.rowid, new.content, new.category);
END;

CREATE INDEX IF NOT EXISTS idx_facts_category   ON facts(category);
CREATE INDEX IF NOT EXISTS idx_facts_confidence ON facts(confidence);
