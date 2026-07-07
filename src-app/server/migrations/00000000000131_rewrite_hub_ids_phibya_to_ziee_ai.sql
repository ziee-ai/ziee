-- Org migration (Phase 2): rebrand the hub publisher namespace
-- `io.github.phibya/*` → `io.github.ziee-ai/*` on existing installs.
--
-- The hub catalog's 5 first-party assistants + 7 first-party models were
-- published under the personal `io.github.phibya` namespace; the org migration
-- moves them to `io.github.ziee-ai`. The seed + catalog now serve the new
-- namespace, so any deployment that installed one of these entities before the
-- rebrand has a `hub_entities.hub_id` that no longer matches the catalog. Without
-- this migration those rows silently drop out of the Updates view + the
-- "installed" badges (the join is by `hub_id`), exactly as pre-§12 slug rows did
-- before migration 92.
--
-- `hub_entities.hub_id` is the ONLY column storing the raw reverse-DNS catalog
-- id; installed assistants/models/mcp_servers reference the catalog only
-- indirectly via `hub_entities.entity_id`, so nothing else needs rewriting.
-- Mirrors migration 92 (the reverse-DNS `hub_id` rewrite) in shape + intent.
--
-- Idempotent: the prefix guard means a second run matches zero rows (rewritten
-- rows already start with `io.github.ziee-ai/`).

DO $$
DECLARE
    rewritten_count int;
BEGIN
    UPDATE hub_entities
    SET hub_id = 'io.github.ziee-ai/' || substring(hub_id from length('io.github.phibya/') + 1)
    WHERE hub_id LIKE 'io.github.phibya/%';

    GET DIAGNOSTICS rewritten_count = ROW_COUNT;
    IF rewritten_count > 0 THEN
        RAISE NOTICE 'hub_entities org migration: rewrote % row(s) io.github.phibya/* -> io.github.ziee-ai/*', rewritten_count;
    END IF;
END $$;
