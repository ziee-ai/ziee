# TEST_RESULTS — chat-power-features

Backend integration (`cargo test --test integration_tests chat::conversation_s`),
backend unit (`cargo test --lib chat::core::repository::conversations::tests`),
frontend unit (`npx tsx --test`), the full `ui` static gate (`npm run check`),
and the enumerated Playwright e2e specs.

## Frontend static gate

- npm run check (ui): PASS

(The diff touches only `src-app/ui/**` product code; `src-app/desktop/ui` is
touched only in its mechanically-generated `openapi.json` + `api-client/types.ts`
— excluded from the frontend-workspace computation — so no desktop `npm run
check` is required.)

## Results

- **TEST-1**: PASS
- **TEST-2**: PASS
- **TEST-3**: PASS
- **TEST-4**: PASS
- **TEST-5**: PASS
- **TEST-6**: PASS
- **TEST-7**: PASS
- **TEST-8**: PASS
- **TEST-9**: PASS
- **TEST-10**: PASS
- **TEST-11**: PASS
- **TEST-12**: PASS
- **TEST-13**: PASS
- **TEST-14**: PASS

### Evidence

- TEST-1/2/4 (search + escape) + TEST-3 (sort): `cargo test ... chat::conversation_s`
  → `4 passed` (search incl. LIKE-metachar regression) + `1 passed` (sort); the
  `normalize_sort` unit → `2 passed`.
- TEST-5/6/7 (chatDrafts / findMatches / collapsible pure helpers): `npx tsx
  --test` → `13 passed`.
- TEST-8..14 (Playwright, `--project=chromium`, `--workers` 1–2): all pass —
  conversation-find, jump-to-latest, collapse-long-message, history-content-search,
  history-sort, composer-draft-persist (2 cases), composer-paste-image.
- Two product fixes surfaced by the e2e (both real regressions the specs caught):
  FIX-25 — `ChatHistoryPage` kept `ConversationList` mounted while a search is
  active (server-side search returns 0 rows on no-match, which otherwise unmounted
  the list + search box and showed the wrong page empty state); FIX-26 — the
  jump-to-latest / auto-scroll `IntersectionObserver` effect re-attaches on
  `conversation?.id` (its `[]` deps meant it bailed during the Loading
  early-return and never observed once the conversation loaded).
