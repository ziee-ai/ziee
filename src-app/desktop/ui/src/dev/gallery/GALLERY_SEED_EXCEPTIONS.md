# Desktop dev-gallery seed exceptions (permanent, committed to main)

The completeness gate (`gen-gallery-seed-registry.mjs --src src --check`, run in
the desktop `npm run check`) fails on any DESKTOP module with a user-facing
surface that does NOT own a `src/modules/<module>/gallery.tsx`, unless listed
HERE. Scans the desktop module tree only; shared web modules are gated by the web
workspace.

Format the gate parses: `- NO-SEED: <module> — <reason> [approved: <who/when>]`

Modules with NO route and NO user-facing slot (desktop-base, file-dialog) are
auto-excluded and need no line.

- NO-SEED: memory — a desktop OVERRIDE of the shared memory module (renders /settings/memory-combined); its endpoints (Memory.list / MemorySettings.get / MemoryAdmin.get) are in the shared crawl base so the page renders populated. A `gallery.tsx` here would raw-shadow `ui/src/modules/memory/gallery.tsx` and fail the override-registry gate — so it is deliberately crawl-covered, not owned. [approved: DEC-B desktop-parity, 2026-07-13]
