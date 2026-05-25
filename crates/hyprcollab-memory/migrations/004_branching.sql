-- Migration 004: Conversation branching + pinned chats

ALTER TABLE messages ADD COLUMN parent_id TEXT REFERENCES messages(id) ON DELETE SET NULL;
CREATE INDEX IF NOT EXISTS idx_messages_parent_id ON messages(parent_id);

ALTER TABLE chats ADD COLUMN pinned BOOLEAN NOT NULL DEFAULT 0;
CREATE INDEX IF NOT EXISTS idx_chats_pinned ON chats(pinned);
