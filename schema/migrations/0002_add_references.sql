-- Add references column to notes table
-- References are stored as JSON array (e.g., source code paths, URLs, etc.)

ALTER TABLE notes ADD COLUMN references TEXT NOT NULL DEFAULT '[]';
