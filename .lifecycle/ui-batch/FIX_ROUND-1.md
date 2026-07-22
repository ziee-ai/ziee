# FIX_ROUND-1 — ui-batch

Round 1 = four blind agents, 16 angles, on `git diff origin/khoi...HEAD`.
Round 2 = two fresh blind agents re-checking the FIXED tree, explicitly told not
to assume round 1's fixes were correct. Full findings in `LEDGER.jsonl`.

## Round 1 — fixed

| # | Finding | Fix |
|---|---|---|
| 1 | **The split fix was incomplete.** `ProjectDetailPage` is `NewChatPage`'s structural twin and navigates to the same `ConversationPage`, so starting a chat from a project reproduced the reported bug exactly. | Reset the workspace there too. |
| 2 | `max-w-[60%]` on the composer's right group RESERVED space unconditionally — the left group is `flex-1` with a zero basis, so it grew into the spare even holding only the "+" button, truncating the model name ~64px early and stranding a gutter. | Replaced the ceiling with a floor on the left group. |
| 3 | ModelSelector's error branch lacked the loaded branch's shrink handling. | Addressed (and corrected again in round 2 — see below). |
| 4 | The sidebar captions became siblings of their `<nav>`, stranding the text outside every landmark. | Rendered as `<h2>`; also restored the 4px caption-to-row rhythm. |
| 5 | **TEST-8's pre-send assertions were vacuous** — `/chat` renders `NewChatPage`, so the split DOM cannot exist there with or without the fix, and the comment claiming "on main the split survives here" was false. | Removed, with the reason recorded. |
| 6 | **TEST-7's row control was a tautology** — it compared three kit `<Menu>` instances built from one shared class string, and the independently-styled recent rows are absent on a fresh account. | Seeds conversations, waits for a real recent row, asserts `recentRowCount >= 1`. |

## Round 2 — fixed (defects in round 1's own fixes)

| # | Finding | Fix |
|---|---|---|
| 7 | The `min-w-9` floor was **wrong twice**: the "+" button is 38px (the kit Button has a 1px transparent border), so the floor was 2px short; and the comment conflated `h-8` (a height) with width. | `min-w-10`. Removing the override entirely was tried and **measured as worse** (Send pushed 150px outside the composer): a flex container's min-content sums children's min-content sizes, and `min-width:0` on the tips does not reduce that contribution. Recorded in the comment so it is not retried. |
| 8 | The error branch's `min-w-0` was a **no-op** — a block child of a block already has min-width 0. The real cause is the kit Button being `inline-flex shrink-0 whitespace-nowrap`, so `max-w-[200px]` never binds and the label runs under Send. | `w-full max-w-[200px] truncate`. |
| 9 | `NewChatPage` still collapsed on MOUNT — the placement round 1 had just rejected for `ProjectDetailPage`. `/` renders the same page and the router bounces every unmatched path (and the 403 "back to home" link) there, so clicking the logo destroyed a split and deleted its persisted workspace. | Moved to the `conversation.created` handler, matching `ProjectDetailPage`. |
| 10 | Two comments stale/wrong: a "60%" bound that no longer exists, and a docstring claiming a div "would leave the text stranded" when this change is what moved the caption out of the `<nav>`. | Rewritten; the heading is now described as a mitigation, with the note that no gate covers it. |

## Round 1 — confirmed but deliberately NOT fixed

Each is recorded in the ledger with its reasoning and surfaced to the human.

- **The ellipsis defect is in the KIT** (`shadcn/select.tsx` sets both
  `line-clamp-1` and `flex` on every trigger), so all ~36 `Select` call sites
  still hard-cut. `sdk/` is a separate submodule repository: fixing it needs its
  own PR plus a pointer bump, and bumping to an unpushed commit would break every
  other clone. **Top follow-up recommendation.**
- **The 12px caption inset and its typography** — the human's explicit decision
  (DEC-4), taken from an option picker showing both alignments with measurements.
  Per the audit-vs-user-decision rule this is recorded and surfaced, never
  silently reversed. The concrete cost the audit measured (the rail now has three
  vertical edges; the settings rail and pane-manager captions still differ) is
  passed to the human rather than absorbed.
- **A reset-vs-async-hydration race** (a reload within the ~250ms debounce window
  can re-hydrate panes after the reset) and **`reset()` clobbering the user's
  tabs-vs-split `mode`** — both belong with the store's contract and its three
  existing callers, not with this fix.

## Verification of the fixes themselves

- `tsc --noEmit` clean after every step.
- TEST-2…TEST-5 re-run green after each change (`5 passed`).
- The new anti-gutter assertion was proven **non-vacuous** by temporarily
  restoring the rejected `max-w-[60%]` ceiling: it fails with *"64.4px of empty
  space sits between the "+" button and an ELLIPSIZED model name"*.
- Round 2's claim that dropping the floor would work was **tested and refuted**
  before being rejected, rather than argued about.

## Convergence

Round 2's findings were all in round 1's fixes or in comment accuracy — no new
defect was found in the ORIGINAL three fixes, which round 2 re-examined from
scratch. Both rounds independently confirmed the same set of clean areas (the
loader hook, the group→flat menu migration, the in-split adopt path, the desktop
pop-out).

**New confirmed findings:** 0

(Superseded: a third round DID follow — the second re-audit agent reported after
this section was written. See FIX_ROUND-2 below.)

---

# FIX_ROUND-2 (round 3 of review)

A second re-audit agent (`reaudit-b`, tests-quality / a11y / responsive /
maintainability / patterns) reported after round 2 had already been committed,
and re-verified its findings against the moved tree rather than the stale diff.

## Fixed

| # | Finding | Fix |
|---|---|---|
| 11 | **The `<h2>` promotion was a bad trade.** The shell renders the sidebar before `<main>` on every page and this app's titles are `Title level={4}` with effectively no `<h1>`, so three sidebar h2s produced an h2,h2,h2,h4 sequence — a skipped level app-wide — to fix a stranded caption confined to the rail. Neither state is gated (axe runs `wcag2a`/`wcag2aa`, excluding `heading-order`; the gallery never renders the sidebar). | Reverted to `<div>`; the tradeoff and the residual are documented in the component. |
| 12 | **`ProjectDetailPage`'s reset had zero coverage at any tier** — deleting it left the unit test, the sidebar e2e and every gate green. | Added `14-split-chat/project-new-chat-collapses-split.spec.ts` (TEST-10). Unrun here for the same docker reason as its siblings. |
| 13 | **Three cross-references still described a mount-time reset** that no longer exists on either page, inviting a future reader to "restore consistency" by hoisting the project reset back to mount — reintroducing the bug. | All three corrected. |
| 14 | The sidebar spec's justification for seeding was wrong: recent-chat rows are NOT independently styled (they render the kit's `MenuRowButton`), so the control detects CONTAINER-inset drift, not row-style drift. | Comment rewritten to state exactly what the control does and does not catch. |
| 15 | A no-op `min-w-0` on both model-selector wrappers, contradicted by the sibling branch's own comment; and a stale "36px" figure in the composer spec. | Both removed/corrected. |

## Confirmed, accepted, not changed

- `SOFT_CEILING_PX = 320` restates `max-w-[20rem]`; it is an upper BOUND (not an
  equality) and its failure message names the ceiling, so a re-tune is a
  one-line test edit. Deriving it would mean importing app code into the visual
  specs, which that suite deliberately avoids.
- TEST-2's light/dark loop asserts only geometry, so the dark run is a
  duplicate. Kept: it costs ~2s and guards against a future theme-dependent
  layout token.
- TEST-5's `opt.scrollW <= opt.clientW` cannot currently fail (the popup is
  `w-auto` and `SelectItem` has no clipping). Kept as a forward guard; the
  load-bearing assertion in that test is that the full name is present while the
  trigger is ellipsized.
- The mic-overflow limit is documented in code and remains unmeasured — the
  gallery cannot render MicButton (it self-hides without the voice capability).

## Convergence

Rounds 2 and 3 found defects only in the FIXES and in comment accuracy — no new
defect in the original three fixes, which both rounds re-examined from scratch.
`reaudit-b` declared its report final ("the source is being edited under me").
Remaining open items are all environmental (unrunnable e2e, the pre-existing
stale registries) rather than review depth.

**New confirmed findings:** 0
