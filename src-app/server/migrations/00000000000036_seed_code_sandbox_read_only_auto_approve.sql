-- Seed read-only sandbox tools into every existing user's
-- auto_approved_tools so common read flows (list files, read files,
-- fetch resource links) don't interrupt the chat loop with manual
-- approval prompts.
--
-- Deliberately scoped to read-only tools:
--   - read_file
--   - list_files
--   - get_resource_link
-- Mutation + execution tools (execute_command, write_file, edit_file)
-- still require explicit user approval the first time.
--
-- Schema (from migration 18):
--   auto_approved_tools JSONB
--   format: [{"server_id": "<uuid>", "tools": ["tool1", "tool2"]}, ...]
--
-- The sandbox server_id matches `code_sandbox_server_id()` in Rust:
--   uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_URL, b"code-sandbox.ziee.internal")
--   = b4d4e17b-55eb-56ce-9bc5-cbc03fd597fd (deterministic; asserted in
--     tier-1 unit test code_sandbox_server_id_is_stable)
--
-- This migration is idempotent at the row level: each user's row gets
-- the sandbox entry only if it isn't already present; if the entry
-- exists with a different tool set, the read-only tools are merged in.
--
-- Note: new users created post-migration inherit the table's default
-- (`'[]'::jsonb`) so they get a one-time prompt for read-only tools
-- too. That's acceptable on first use.

DO $$
DECLARE
    sandbox_id CONSTANT TEXT := 'b4d4e17b-55eb-56ce-9bc5-cbc03fd597fd';
    read_only_tools CONSTANT JSONB := '["read_file", "list_files", "get_resource_link"]'::jsonb;
    rec RECORD;
    updated JSONB;
    found_idx INT;
    existing_tools JSONB;
    merged_tools JSONB;
BEGIN
    FOR rec IN SELECT id, auto_approved_tools FROM user_mcp_defaults LOOP
        -- Find existing entry for the sandbox server, if any.
        found_idx := NULL;
        FOR i IN 0..jsonb_array_length(rec.auto_approved_tools) - 1 LOOP
            IF rec.auto_approved_tools -> i ->> 'server_id' = sandbox_id THEN
                found_idx := i;
                EXIT;
            END IF;
        END LOOP;

        IF found_idx IS NULL THEN
            -- No entry — append.
            updated := rec.auto_approved_tools
                     || jsonb_build_array(
                         jsonb_build_object('server_id', sandbox_id, 'tools', read_only_tools)
                     );
        ELSE
            -- Entry exists — merge read_only_tools into the existing
            -- tools array, preserving uniqueness.
            existing_tools := COALESCE(rec.auto_approved_tools -> found_idx -> 'tools', '[]'::jsonb);
            merged_tools := existing_tools;
            FOR i IN 0..jsonb_array_length(read_only_tools) - 1 LOOP
                IF NOT merged_tools @> jsonb_build_array(read_only_tools -> i) THEN
                    merged_tools := merged_tools || jsonb_build_array(read_only_tools -> i);
                END IF;
            END LOOP;
            updated := jsonb_set(
                rec.auto_approved_tools,
                ARRAY[found_idx::text],
                jsonb_build_object('server_id', sandbox_id, 'tools', merged_tools)
            );
        END IF;

        UPDATE user_mcp_defaults
        SET auto_approved_tools = updated,
            updated_at = NOW()
        WHERE id = rec.id
          AND auto_approved_tools IS DISTINCT FROM updated;
    END LOOP;
END $$;
