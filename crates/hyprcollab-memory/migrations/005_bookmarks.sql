-- Migration 005: Message bookmarks

CREATE TABLE IF NOT EXISTS bookmarks (
    id         TEXT PRIMARY KEY,
    chat_id    TEXT NOT NULL REFERENCES chats(id) ON DELETE CASCADE,
    message_id TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    label      TEXT,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_bookmarks_chat_id    ON bookmarks(chat_id);
CREATE INDEX IF NOT EXISTS idx_bookmarks_message_id ON bookmarks(message_id);
