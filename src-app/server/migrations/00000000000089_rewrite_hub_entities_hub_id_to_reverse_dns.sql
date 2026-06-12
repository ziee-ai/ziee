-- Phase 7 / §13.6: rewrite slug `hub_id` values to reverse-DNS.
--
-- Pre-§12 installs stored a slug (e.g. "filesystem-mcp",
-- "code-reviewer", "llama-3-8b-instruct"). Post-§12, the catalog uses
-- reverse-DNS (e.g. "io.github.modelcontextprotocol/filesystem"). The
-- Updates view + the "installed" badges join hub_entities against the
-- catalog by hub_id; without this migration, every pre-§12 install row
-- silently drops out of those views.
--
-- The lookup table below was derived from the v1 ziee-ai/hub catalog
-- releases. Slugs not in the table are left as-is and surface as a
-- NOTICE — the user can reinstall those manually.
--
-- Idempotent: rows whose hub_id already contains `/` are skipped
-- (already reverse-DNS).

UPDATE hub_entities AS h
SET hub_id = m.new_hub_id
FROM (VALUES
    -- MCP servers
    ('filesystem-mcp',          'io.github.modelcontextprotocol/filesystem'),
    ('memory-mcp',              'io.github.modelcontextprotocol/memory'),
    ('postgres-mcp',            'io.github.modelcontextprotocol/postgres'),
    ('github-mcp',              'io.github.github/mcp'),
    ('brave-search-mcp',        'com.brave/search-mcp'),
    ('linear-mcp',              'app.linear/mcp'),
    -- Models
    ('llama-3-1-8b-instruct',           'io.github.phibya/llama-3-1-8b-instruct'),
    ('llama-3-2-3b-instruct-gguf',      'io.github.phibya/llama-3-2-3b-instruct-gguf'),
    ('qwen2.5-coder-7b-instruct',       'io.github.phibya/qwen2.5-coder-7b-instruct'),
    ('qwen2.5-vl-3b-instruct',          'io.github.phibya/qwen2.5-vl-3b-instruct'),
    ('phi-3-mini-4k-instruct',          'io.github.phibya/phi-3-mini-4k-instruct'),
    ('nomic-embed-text-v1-5-gguf',      'io.github.phibya/nomic-embed-text-v1-5-gguf'),
    ('deepseek-r1-70b',                 'io.github.phibya/deepseek-r1-70b'),
    -- Assistants
    ('code-reviewer',   'io.github.phibya/code-reviewer'),
    ('creative-writer', 'io.github.phibya/creative-writer'),
    ('deep-researcher', 'io.github.phibya/deep-researcher'),
    ('sql-helper',      'io.github.phibya/sql-helper'),
    ('vision-analyst',  'io.github.phibya/vision-analyst')
) AS m(old_slug, new_hub_id)
WHERE h.hub_id = m.old_slug
  AND h.hub_id NOT LIKE '%/%';

-- Emit a notice for slug-shaped rows we couldn't map. They survive the
-- migration unchanged; the user reinstalls them to re-track.
DO $$
DECLARE
    orphan_count int;
BEGIN
    SELECT COUNT(*) INTO orphan_count
    FROM hub_entities
    WHERE hub_id NOT LIKE '%/%';
    IF orphan_count > 0 THEN
        RAISE NOTICE 'hub_entities migration: % row(s) have unrecognized slug-style hub_id (left untouched; reinstall to re-track)', orphan_count;
    END IF;
END $$;
