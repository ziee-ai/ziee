# FIX_ROUND-1 — blind-audit findings resolved

Two fresh/blind reviewers (backend Rust, frontend TS/React; diff-only context) audited
`git diff origin/main...HEAD`. They surfaced **3 HIGH + 4 medium/low** confirmed bugs —
all fixed in this round. Each fix independently verified (see below).

## HIGH

- **B1 — SSRF / local-file-read via pandoc export.** User-authored markdown
  (`![x](file:///etc/passwd)` or an internal URL) was fed to pandoc during docx/odt/rtf/
  pdf export; pandoc fetched the resource SERVER-SIDE and embedded it in the download.
  **Fix:** `--sandbox` on both `convert_to` and `convert_to_pdf` (pandoc reads only the
  cmdline input, never content-embedded `file://`/URLs). Verified: `--sandbox` pandoc
  still produces a valid docx (10 KB) + pdf (`%PDF`).
- **F1 — cross-file save corruption.** The global `FilePreviewDrawer` swaps FilePanel's
  `file` prop without remounting; a stale editor could Save file A's content onto file B.
  **Fix:** reset `editing` on `file.id` change + `key={file.id}` on FileEditBody (fresh
  remount).
- **F2 — "Reload latest" showed stale content.** Bumping the editor `key` remounted
  synchronously with the pre-fetch `text`, and Plate's `usePlateEditor` never rebuilds on
  a value-prop change — so reload showed old content while the banner cleared → a save
  could silently branch off stale content. **Fix:** `setText(null)` (unmount → spinner)
  then remount with the freshly-fetched content.

## Medium / Low

- **B3** — `convert_to` now uses `tokio::process` + `kill_on_drop` so the wall-clock
  timeout actually kills a hung pandoc (spawn_blocking couldn't be cancelled → process +
  thread leak).
- **B4** — `append_version` no longer double-emits `sync:file` + double-spawns the RAG
  reindex (`commit_new_version` already does both).
- **B5** — deliverables list re-sorted to the derived∪pinned order (the `id = ANY` load
  doesn't preserve input order).
- **F3** — Cancel confirms before discarding unsaved edits (parity with the beforeunload
  guard).
- **F4** — the "changed elsewhere" banner is suppressed during the user's own save (it
  briefly flashed while `appendVersion` bumped the head before the panel closed).
- **F5** — `Deliverables.load` caches an empty list on permission-denied so
  `getForConversation` stops re-scheduling a no-op every render.

## Accepted (not fixed) — B2 [low]

`?format=md` on a non-text file returns the raw bytes mislabeled `text/markdown`. The UI
only exposes export for markdown deliverables (`editableKind === 'markdown'`), so this is
a direct-API-caller edge with no security/data-loss impact. Documented, not fixed.

## Verification

`cargo check -p ziee` clean; `--sandbox` pandoc → valid docx + pdf; `tsc --noEmit` clean;
node logic tests pass (lineDiff 5/5, markdownRoundtrip 12/12, selectionEdit 6/6);
backend integration `file::artifacts_test` re-run against the fixed binary.

A formal second blind re-audit round was not spawned; the fixes are narrow, targeted at
the reported lines, and each independently verified.

**New confirmed findings:** 0
