# BASE — conflict-surface scoping

- **Base branch:** `origin/khoi` (worktree cut from it). Run the gate with
  `--base origin/khoi`.
- **Highest existing migration:** `00000000000153_scheduled_task_unattended_tools.sql`.
  This change adds **NO migration** → zero migration-number collision surface.
- **Files this branch touches vs. what main is changing:**
  - `docker/web/nginx.conf` — infra config; not a hot file in active feature work.
  - `docker/web/check-sse-headers.mjs` (new), `docker/web/README.md` — new/doc.
  None are under `src-app/**`, so there is **no Rust/TS build surface**, no
  `openapi.json` regen, and no desktop-`ui/` override to mirror.
- **OpenAPI regen implied?** No — no API types change.
- **Collision risk:** minimal; a concurrent worker editing `docker/web/nginx.conf`
  is the only realistic overlap. (The other active worktree, `resource-link-ssrf`,
  is backend-only — no `docker/web/` overlap.)
