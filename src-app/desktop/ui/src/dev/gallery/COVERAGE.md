# Desktop seeded gallery — coverage checklist

Mirrors the web gallery (`../../../ui/src/dev/gallery`) with the DESKTOP
api-client types + `openapi.json` + module set. Desktop reuses the web core via
the `@/` override plugin, so the gallery loads BOTH core modules (`loadModules`)
and desktop-specific ones (`loadDesktopModules`) — the same bootstrap as the real
desktop app.

## Coverage (19 desktop-own surfaces — the delta over the shared web core)

| Kind | Count | Meaning |
|---|---:|---|
| `page` | 6 | Desktop route element (host-mount, remote-access, memory-combined, about, magic-link, phone-auth). |
| `via` | 9 | Rendered within the desktop app-layout chrome / settings shell / a page. |
| `allow` | 2 | Non-visual (auth guard, project-extension registration). |
| `pending` | 2 | Interaction-only (desktop Drawer, conversation host-mount control). |

The shared web surfaces (kit, core modules) are covered by the web gallery gate.

## Render

44 routes enumerated (core + desktop-specific) render via mock-API; the
desktop-only pages (`/settings/remote-access`, `/settings/host-mount`,
`/settings/memory-combined`, `/settings/about`, `/auth/magic`) render populated.

## Fixtures / gates (desktop-scoped)

- Recorded from the shared backend, filtered to the desktop client's endpoints.
- Typed vs desktop `types.ts` (tsc); ajv-validated vs desktop `openapi.json`
  (62 values, 0 fatal).
- `npm run check` (desktop) runs check:gallery-coverage + check:gallery-crawl +
  gallery:check-fixtures.
