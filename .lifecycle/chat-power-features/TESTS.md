# TESTS — chat-power-features

Every ITEM is covered by ≥1 test. All UI-touching items carry a `tier: e2e`
spec. Backend items carry unit (pure ORDER BY whitelist) + integration
(real HTTP + DB) tests. UI unit tests use the repo's `node --test`
(`src/**/*.test.ts`) runner (see `core/tool-status.test.ts`); pure helpers are
extracted so the logic is unit-testable.

## Backend

- **TEST-1** (tier: integration) [covers: ITEM-4] file: `src-app/server/tests/chat/conversation_search_test.rs` — asserts: `GET /conversations?search=<term>` returns a conversation whose MESSAGE TEXT contains the term even though its title does not, and excludes a conversation matching neither title nor content.
- **TEST-2** (tier: integration) [covers: ITEM-4] file: `src-app/server/tests/chat/conversation_search_test.rs` — asserts: with `search` set, the response `total` equals the FILTERED count (not the unfiltered total), so pagination is consistent; title-substring matches are also returned (title OR content).
- **TEST-3** (tier: integration) [covers: ITEM-5] file: `src-app/server/tests/chat/conversation_sort_test.rs` — asserts: `sort=oldest` / `alpha` / `most_messages` / `recent` return the seeded conversations in the expected order, and an unknown `sort` value falls back to `recent` (updated_at DESC).
- **TEST-4** (tier: unit) [covers: ITEM-5] file: `src-app/server/src/modules/chat/core/repository/conversations.rs` — asserts: the pure `normalize_sort` whitelist maps each known key (`recent`/`oldest`/`alpha`/`most_messages`) to itself and any other/None input to the `recent` default, so only whitelisted keys ever reach the query's ORDER BY CASE (guards against injection + unknown input).

## Frontend — unit

- **TEST-5** (tier: unit) [covers: ITEM-7] file: `src-app/ui/src/modules/chat/extensions/text/chatDrafts.test.ts` — asserts: `getDraft/setDraft/clearDraft` roundtrip against a stubbed `localStorage`, empty value removes the key, and clearing a conversation draft also clears the `new` bucket.
- **TEST-6** (tier: unit) [covers: ITEM-1] file: `src-app/ui/src/modules/chat/components/findMatches.test.ts` — asserts: `findMatches(messages, query)` returns message ids in display order, is case-insensitive, matches only `text` content, and returns `[]` for a blank/whitespace query.
- **TEST-7** (tier: unit) [covers: ITEM-3] file: `src-app/ui/src/modules/chat/components/collapsible.test.ts` — asserts: the pure `shouldOfferCollapse({ length, isStreaming })` returns true only for non-streaming content past the char threshold and false while streaming or under threshold.

## Frontend — e2e

- **TEST-8** (tier: e2e) [covers: ITEM-1] file: `src-app/ui/tests/e2e/chat/conversation-find.spec.ts` — asserts: in a seeded conversation the user opens the find bar, types a term present in one message, sees a "1 of N" match count, and Next scrolls that message into view with the highlight applied; Esc closes and clears.
- **TEST-9** (tier: e2e) [covers: ITEM-2] file: `src-app/ui/tests/e2e/chat/jump-to-latest.spec.ts` — asserts: in a long seeded conversation the jump-to-latest button is hidden at bottom, appears after scrolling up, and on click returns the view to the latest message and hides again.
- **TEST-10** (tier: e2e) [covers: ITEM-3] file: `src-app/ui/tests/e2e/chat/collapse-long-message.spec.ts` — asserts: a seeded very-long message renders clamped with a "Show more" control; clicking expands to full height (and "Show less" re-clamps).
- **TEST-11** (tier: e2e) [covers: ITEM-6] file: `src-app/ui/tests/e2e/chat/history-content-search.spec.ts` — asserts: search on `/chats` resolves SERVER-SIDE — a unique-titled conversation seeded beyond the first page (so the client hasn't loaded it) is surfaced by searching its term, proving the query hits the backend rather than the old client-only filter; a non-matching term shows the empty state. (Content-vs-title matching itself is covered against a real DB by TEST-1/TEST-2.)
- **TEST-12** (tier: e2e) [covers: ITEM-6, ITEM-5] file: `src-app/ui/tests/e2e/chat/history-sort.spec.ts` — asserts: on `/chats`, changing the sort control from Recent to Oldest (and Alphabetical) reorders the visible conversation list accordingly.
- **TEST-13** (tier: e2e) [covers: ITEM-7] file: `src-app/ui/tests/e2e/chat/composer-draft-persist.spec.ts` — asserts: typed but unsent composer text survives navigating away and back to the same conversation, and is cleared after the message is sent.
- **TEST-14** (tier: e2e) [covers: ITEM-8] file: `src-app/ui/tests/e2e/chat/composer-paste-image.spec.ts` — asserts: dispatching a clipboard paste carrying an image file onto the composer adds it as a pending attachment (an attachment preview appears), while pasting plain text does not.
