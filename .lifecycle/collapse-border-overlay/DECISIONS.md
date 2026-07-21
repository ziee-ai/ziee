# DECISIONS — collapse-border-overlay

Every human/product input the implementation needs, resolved before coding.
No open markers remain.

### DEC-1: Does the fix restructure what the clamp contains, or only give the ring room? [REVISED after the blind audit]
**Resolution:** **Only give the ring room.** `CollapsibleBlock` keeps clamping the
WHOLE bubble exactly as before; the sole change is a 2px horizontal inset on the
clamped container. `ChatMessage.tsx` is not modified at all.

**Superseded first resolution:** split at the last structural node — hoist
thinking/tool cards above the fold and clamp only the trailing prose. The user
approved this from an option picker at plan time, and it was implemented.

**Why it was reversed:** the phase-6 blind audit (three independent auditors,
converging) found it caused two HIGH regressions, and I reproduced one. The
collapse DECISION is computed over the whole message
(`shouldOfferCollapse(messageText(message).length)`) while the split made the
clamp SCOPE only the trailing prose, so the two diverge:
- `[long text] → [tool_use]` — no trailing prose, so `clampedNodes` is empty and
  nothing clamps. **Measured: 1044px tall, no collapsible, no toggle**, where it
  previously clamped to 384px.
- `[long text] → [tool_use] → [short text]` — the clamp wraps only the short tail,
  measures no overflow, and the toggle disappears while the long prefix renders
  full height.
Both are common agentic turn shapes. `estimateMessageHeight` is also clamp-unaware,
so the virtualizer would flip from a bounded over-estimate to an unbounded
under-estimate — the direction that makes the scrollbar jump.

Critically, the reproduction had already shown the inset ALONE fully fixes the
reported bug (rings 0 → 25 with mask and clip both still active), so the split was
never load-bearing. It only removed a secondary ~4% ramp attenuation on cards
below the fold.
**Basis:** user — the regression evidence was presented as an explicit option
picker rather than silently reversing their earlier choice (the
audit-vs-user-decision rule). The user chose to drop the split and ship the inset
alone. See DRIFT-2.md.

> **DEC-2, DEC-3 and DEC-4 are MOOT** — they resolved details of the split
> (which content types are structural, how a multi-block node is classified, what
> happens when the prose suffix is empty). With DEC-1 reversed there is no split,
> no `classifyNode`, and no `splitTrailingProse`; `ChatMessage.tsx` is unchanged.
> Retained unedited as the record of what was decided before the audit.

### DEC-2: Which content types count as "structural" (hoisted) vs "prose" (clamped)? [MOOT — see DEC-1]
**Resolution:** Structural = `thinking | tool_use | tool_result | file_attachment`
— exactly the kinds that render a bordered kit `<Card>` / `FileCard`. Prose =
`text | image`. An assistant `image` block stays INSIDE the clamp and fades
normally; it is content, not a process affordance, and it renders no ring.
**Basis:** codebase — the content-type union is enumerated at
`chat/core/extensions/types.ts:299`; the card-rendering kinds are
`ThinkingContent.tsx:28` (thinking) and `mcp/chat-extension/extension.tsx:184,332`
(tool_use/tool_result). Keeping the hoist set as narrow as possible preserves the
existing fade semantics for everything that is genuinely prose.

### DEC-3: How is a node's kind determined when one renderer consumes several blocks?
**Resolution:** Tag each EMITTED node structural if **any** block it consumed is
non-`text` — not by inspecting only the starting block.
**Basis:** codebase — today `collectToolRun`
(`mcp/chat-extension/extension.tsx:252-257`) breaks on the first
non-`tool_use`/`tool_result` block, so a consumed run is always purely structural
and starting-block inspection would be sufficient. Tagging over the whole consumed
span is defensive: a future `contentSpan` that spans a text block cannot silently
mis-split the message. Cost is one loop over an already-available slice.

### DEC-4: What happens when the trailing prose suffix is empty (a turn ending on a tool card)?
**Resolution:** Skip the `CollapsibleBlock` wrapper entirely — render the nodes
plainly. Do not emit an empty wrapper div.
**Basis:** convention — mirrors the existing `bubbleBlocks.length > 0` guard at
`ChatMessage.tsx:207`, which already refuses to render an empty bubble. An empty
wrapper would measure `scrollHeight` 0 and render no toggle, so it is inert, but
emitting dead DOM is not the house style.

### DEC-5: How is the `overflow-hidden` clip stopped from shaving rings, without shifting text?
**Resolution:** `-mx-0.5 px-0.5` applied UNCONDITIONALLY to the masked content
container. Equal negative margin + padding leaves the content box byte-identical
while pushing the clip edge 2px outward.
**Basis:** convention — this is the same problem the parent already solved at
`ChatMessage.tsx:228-231` with `px-0.5` ("so a full-width child Card's left/right
border + rounded corners aren't shaved by the clip"). Two alternatives were
rejected: a bare `px-0.5` would inset prose text 2px per side on top of the
parent's inset; gating it on `isClamped` would change text wrapping between
collapsed and expanded, producing a visible reflow on every toggle. The 4px
growth is absorbed exactly by the parent's own `px-0.5`, so outer geometry is
unchanged.

### DEC-6: Is the mask gradient itself re-tuned (e.g. ramp over 48px instead of 96px)?
**Resolution:** No. `[mask-image:linear-gradient(to_bottom,black_75%,transparent)]`
is left byte-identical, and `CollapsibleBlock`'s behavior (ResizeObserver,
`data-collapsed`, `onFocusCapture`, `useInPlaceAnchor`, store-lifted flag) is
untouched. The only edit to that file is DEC-5's inset.
**Basis:** user + codebase — the task constraints require the fade stay a MASK
rather than a color overlay so it blends over any background with no color band
(the rationale is recorded in the code comment at `CollapsibleBlock.tsx:114-116`).
Re-tuning the ramp would be a mitigation that still leaves cards faded on a
tall-enough card run, and would not address Mechanism A at all.

### DEC-7: Does this feature introduce an operational tunable that must be admin-configurable?
**Resolution:** No — no new tunable is introduced, so no settings row, migration,
permission, or admin card is required. `COLLAPSE_MAX_HEIGHT_PX` (384) and
`COLLAPSE_CHAR_THRESHOLD` (1200) are PRE-EXISTING named constants in
`collapsible.ts:13,21` and are not changed by this work. The 2px inset (DEC-5) is
a layout detail, not an operator-facing knob.
**Basis:** convention — the mandatory configurable-settings rule applies to
resource limits, retention, quotas, concurrency, toggles, and model selection.
A fixed presentational clamp height is none of these, and the existing constants
are already extracted (not inline magic numbers), so they can be promoted later
without a rewrite if a product need appears.

### DEC-8: What is the durable regression pin, given Layer B baselines are gitignored?
**Resolution:** Deterministic DOM/geometry assertions (TESTS.md TEST-2/3/4/8), not
pixel snapshots. Layer B is run locally to eyeball the result but is not the
committed guarantee.
**Basis:** codebase — `src-app/ui/.gitignore:36` ignores
`tests/e2e/visual/**/*-snapshots/`, so blessed screenshots do not ride the PR and
could not gate a reviewer's machine.

- DESCOPED: ITEM-2 — the `splitTrailingProse` / `classifyNode` helpers have no consumer once the split is reverted; both files restored to base. [approved: user — option-picker at phase 6/7 convergence, "Drop the split; ship the inset alone"]
- DESCOPED: ITEM-3 — the structural/prose split is reverted in full after the blind audit proved two HIGH regressions (a long turn ending on a structural node loses height-bounding entirely; measured 1044px, no toggle), and the reproduction had already shown ITEM-4 alone fixes the reported bug. [approved: user — option-picker at phase 6/7 convergence, "Drop the split; ship the inset alone"]

### DEC-9: Which base ref do the lifecycle gates and the PR use?
**Resolution:** `origin/khoi` for every `lifecycle-check.mjs` invocation
(`--base origin/khoi`); the PR targets `khoi`.
**Basis:** user — the task specifies branching off `origin/khoi` and PR'ing into
`khoi` because this is a platform-wide UI fix. The validator defaults to
`origin/main`, which is 382 commits behind khoi and would make the Phase-6
coverage law meaningless. Recorded in BASE.md.
