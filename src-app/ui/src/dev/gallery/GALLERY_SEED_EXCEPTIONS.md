# Dev-gallery seed exceptions (permanent, committed to main)

The completeness gate (`gen-gallery-seed-registry.mjs --check`, run in
`npm run check`) fails on any module with a user-facing surface (a non-skip route
`path:` or a user-facing slot) that does NOT own a
`src/modules/<module>/gallery.tsx`, unless it is listed HERE.

This file is the PERMANENT source of truth for those approvals. It lives in the
product tree (NOT under `.lifecycle/`, which is stripped at merge) so the gate
keeps working on `main` for everyone.

A module needs a line here ONLY when the surface heuristic flags it but a
`gallery.tsx` is genuinely inappropriate (an infra/redirect shell). Modules with
NO route and NO user-facing slot (config-client, layouts, router, dev-gallery)
are auto-excluded and need no line. A module fully covered by the shared crawl
should instead ship a `gallery.tsx` exporting `{ crawlOnly: true }` — an explicit
ownership marker — not a no-seed exception.

Format the gate parses (one per line):
`- NO-SEED: <module> — <reason> [approved: <who/when>]`

<!-- No exceptions currently: every surface-bearing module owns a gallery.tsx. -->
