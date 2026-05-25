-- Migration 006: FTS5 full-text search across messages

CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(
    content,
    role,
    content=messages,
    content_rowid=rowid
);

-- Keep FTS5 index in sync with the messages table
CREATE TRIGGER IF NOT EXISTS messages_ai AFTER INSERT ON messages BEGIN
    INSERT INTO messages_fts(rowid, content, role)
    VALUES (new.rowid, new.content, new.role);
END;

CREATE TRIGGER IF NOT EXISTS messages_ad AFTER DELETE ON messages BEGIN
    INSERT INTO messages_fts(messages_fts, rowid, content, role)
    VALUES ('delete', old.rowid, old.content, old.role);
END;

CREATE TRIGGER IF NOT EXISTS messages_au AFTER UPDATE ON messages BEGIN
    INSERT INTO messages_fts(messages_fts, rowid, content, role)
    VALUES ('delete', old.rowid, old.content, old.role);
    INSERT INTO messages_fts(rowid, content, role)
    VALUES (new.rowid, new.content, new.role);
END;

-- Populate index for any messages that existed before this migration
INSERT INTO messages_fts(messages_fts) VALUES ('rebuild');
