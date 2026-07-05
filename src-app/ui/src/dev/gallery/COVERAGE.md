# Seeded gallery — coverage checklist

Tracks what the seeded gallery covers, per workspace. Updated as the fan-out
lands. See `SEEDED_GALLERY_PLAN.md` for the approach.

## Web UI (`src-app/ui`)

| Dimension | Covered | Total | Notes |
|---|---|---|---|
| Pages | 1 | ~35 | llm-providers (populated). Fan-out pending sign-off. |
| Stores | (via page) | ~130 | Seeded through the mock-API load path. |
| Components | (via page) | ~209 | Rendered inside seeded pages + kit stories. |

### Vertical slice (done)

- **Settings · LLM Providers** — real recorded providers (7 built-in, 4 enabled)
  + 9 models + group-assignment card, replayed via mock-API. Fixture is
  recorded-from-server + typed (`tsc`) + contract-validated (ajv vs
  `openapi.json`).

## Desktop UI (`src-app/desktop/ui`)

Pending — mirror the web harness with the desktop `types.ts` + `openapi.json`,
covering desktop-only surfaces (auto-login, updater).

## Fixture correctness (all layers green for the slice)

1. Typed against `src/api-client/types.ts` — `npx tsc --noEmit`.
2. Recorded from a real server — `npm run gallery:record`.
3. Contract-validated — `npm run gallery:check-fixtures` (ajv vs `openapi.json`).
