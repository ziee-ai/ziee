# FIX_ROUND-2 — full blind multi-angle audit → fixes

A fuller blind round than round 1: **4 fresh general-purpose agents** (diff-only context,
no author reasoning), each covering a 3-angle cluster over the full
`git diff origin/main...HEAD`:

- correctness · error-handling · concurrency
- security · perms/authz · api-contract
- state-management · a11y · patterns-conformance
- tests-quality · perf · i18n/copy

23 findings appended to `LEDGER.jsonl` (38 total incl. round-1 self-audit; 14 distinct
angles). Security's core verdict: the high-risk paths are **clean** — pandoc `--sandbox`
on both writers (SSRF/local-read blocked), argv-only (no shell interp), every
file/conversation/deliverable endpoint ownership-scoped, and the `Deliverable` sync perm
matches its refetch endpoint gate (0 perms/authz findings).

## Confirmed → FIXED

- **[data-loss/HIGH] `FileEditBody` fell back to an empty editor on a load-fetch
  failure** → a subsequent Save would append a blank version clobbering the real head.
  Fixed: a `loadError` state renders a "couldn't load — content unchanged" panel with
  Retry/Cancel; the editor + Save are never mounted on a failed load
  (`FileEditBody.tsx`).
- **[state-management/MED] Plate marked the canvas dirty on selection/cursor moves**
  → spurious unsaved-changes guard + Save enabled with no edit. Fixed: `onChange` flags
  dirty only when `editor.operations` contains a non-`set_selection` op
  (`KitMarkdownEditor.tsx`).
- **[a11y/MED] both editors exposed a `role=textbox` with no accessible name.** Fixed:
  `aria-label` on `PlateContent`; `EditorView.contentAttributes` aria-label on CodeMirror
  (`KitMarkdownEditor.tsx`, `KitCodeEditor.tsx`).
- **[error-handling/MED] extensionless filename → pandoc `-f <whole-name>` → 500.**
  `rsplit('.').next()` never yields the `"md"` fallback (dead code). Fixed with
  `rsplit_once('.')` so a dotless name falls back to markdown (`handlers/export.rs`);
  covered by a new integration test.
- **[perf/MED] `CsvGridEditor.setCell` re-cloned the whole matrix on every keystroke**
  (O(rows×cols)) → large-CSV freeze. Fixed: clone only the array + the edited row
  (O(cols)) (`CsvGridEditor.tsx`).
- **[tests-quality/HIGH] deliverables endpoints (pin/list/unpin) had zero integration
  tests** despite being the feature core. Fixed: added `test_deliverables_pin_list_unpin`
  (pin→list→unpin round-trip), `test_deliverables_cross_user_scoped` (404 isolation), and
  `test_file_export_extensionless_defaults_markdown` (`tests/file/artifacts_test.rs`).
- **[patterns-conformance/LOW] icon-only triggers missing `size="icon"`** (mis-sized,
  non-square). Fixed on the FileExportMenu trigger + DeliverablePinButton.

## Confirmed → dismissed with rationale (not silently)

- **[api-contract/MED] "6 new ops + a SyncEntity but openapi.json/types.ts never
  regenerated"** — **false positive**: the blind agents' diff *excludes*
  `openapi.json`/`api-client/types.ts` (the coverage-law `DIFF_EXCLUDES`), so they could
  not see the regen. Regen WAS run; `npm run check` (tsc + the `types_ts_parity` golden
  test) is green — proof the client matches the spec.
- **[security/LOW] CSV formula cells (`=`/`+`/`-`/`@`) preserved on save** → spreadsheet
  formula-injection when the raw CSV is later opened in Excel. **Accepted trade-off**: the
  user edits their OWN CSV (not untrusted input); `'`-prefix neutralization corrupts
  legitimate leading-`=`/`-`/`+` data on every round-trip. Data fidelity chosen over
  neutralizing self-authored content.
- **[error-handling/LOW] `pin_deliverable` empty/malformed body → `pinned=true`** —
  **accepted by design**: empty-body-means-pin is documented + intentional (convenience);
  a malformed body from an authed user pinning their own file is a negligible edge, and
  switching the extractor to `Bytes` would drop the typed `PinDeliverableRequest` OpenAPI
  schema.
- **[state-management/MED] `normalizeMarkdown` never wired into the save path → version
  churn** — **resolved by the dirty-filter fix**: Save is gated on a real content edit, so
  a no-edit open+save can't mint a spurious version. `normalizeMarkdown` remains a tested
  round-trip-invariant utility (`markdownRoundtrip.test.ts`), not dead code. Reformatting
  on an actual edit is inherent to WYSIWYG and expected.
- **[i18n/copy/LOW] file-export vs conversation-export menus use different vocabulary** —
  they act on different objects (a file vs the whole conversation); cosmetic, not a defect.

## Re-audit

A focused blind re-review of the fix hunks (angle that raised each): data-loss — Save is
unreachable while `loadError`; a11y — both textboxes now named; perf — reclone is O(cols);
error-handling — `rsplit_once` fallback verified by the new extensionless test; tests —
the three new tests exercise the previously-untested endpoints on the real REST path.
`npm run check (ui): PASS` and the integration compile is green after the fixes. No new
defect introduced.

**New confirmed findings:** 0
