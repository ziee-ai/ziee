-- Add fork_level to branches to distinguish edit ('user') vs regenerate ('assistant') flows.
-- This allows the frontend to correctly anchor the branch navigator after page reload,
-- without relying on in-memory state that is lost on refresh.
ALTER TABLE branches
    ADD COLUMN fork_level TEXT NOT NULL DEFAULT 'user'
        CHECK (fork_level IN ('user', 'assistant'));
