# Crop-review manifest (GENERATED — Layer 2)

> `node scripts/gen-crop-review-manifests.mjs`. Vision review happens on **component crops at native resolution** (process-rule 4). Each crop under `crops/` is reviewed against the rubric below. The rubric IS the taxonomy's `[V]` (vision) classes, parsed live from `docs/DEFECT_TAXONOMY.md` so it can never drift. **Process-rule 3: answer the ABSENCE questions, not only "what looks wrong".**

## The vision rubric (every `[V]` taxonomy class)

### A. Spacing & adjacency
- [ ] A5 — padding asymmetric where design implies symmetry (card L≠R, modal header vs footer)
- [ ] A6 — insufficient whitespace between unrelated sections / crowding

### B. Wrap, reflow & responsive
- [ ] B4 — wrong stacking order after reflow (actions land above their label; controls disconnected from their row)
- [ ] B5 — element unusable/cut at 375px (tap targets, buttons half-visible)
- [ ] B7 — desktop dead-gutter / content not using width per the app's standard settings max-width

### C. Order & composition semantics
- [ ] C1 — status tag/badge ordered before its label/key — badges follow the thing they qualify. (user miss #4: "(verified) vaswani2017attention")
- [ ] C2 — icon on the wrong side per app convention (leading icons for nav, trailing for external/chevrons)
- [ ] C3 — primary/secondary button order inconsistent with the app convention (and OS-idiom per platform)
- [ ] C4 — orphaned/floating control — a control visually disconnected from what it acts on
- [ ] C5 — duplicate signals (banner + redundant toast for the same event)
- [ ] C6 — mixed alignment within one group (some rows left, one centered)
- [ ] C8 — missing differentiation affordance (broader judgment form of C7): any list/thread mixing entity kinds where a reader cannot tell kinds apart at a glance
- [ ] C13 — valueless decoration — an element that consumes space but adds no information or affordance (an avatar in a two-party chat where the parties are/should be distinguished by layout; repeated decorative icons; badges that are always the same value). Rubric question: "what would be lost if this element were removed?" — if nothing, flag. (user miss #13b: the user avatar has no value even if filled)
- [ ] C11 — icon-action semantic mismatch — the icon glyph does not communicate its action. Lintable core: an action-name→expected-icon table (open-in-new-tab→ExternalLink, download→Download, delete→Trash2, copy→Copy, edit→Pencil, settings→Settings, close→X, refresh→RotateCw...); AST lint compares a control's aria-label/tooltip/action text with its imported icon and flags mismatches. Vision rubric covers unlabeled icons. (user miss #10b: the open-in-new-tab icon doesn't read as open-in-new-tab)

### D. Truncation & content fit
- [ ] D2 — truncation without ellipsis/title-tooltip affordance (hard clip)
- [ ] D3 — truncation of the DISTINGUISHING part (IDs/keys truncated so items look identical)
- [ ] D5 — numeric formatting broken — NaN/undefined/Infinity/raw 0 rendered ("NaN GB")
- [ ] D6 — widows/orphans in headings; single-word wrapped lines in buttons

### E. Typography
- [ ] E2 — hierarchy inversion (child heading visually heavier than parent)
- [ ] E4 — baseline misalignment of inline icon+text pairs

### F. Color, theme & contrast
- [ ] F2 — dark-mode-specific invisibility (borders/swatches/dividers vanish; the black-swatch class)
- [ ] F4 — state colors misused (success/danger/warning semantics)
- [ ] F5 — disabled-look on enabled controls (the desaturated-primary class)

### G. Interaction states (per interactive component)
- [ ] G1 — focus-visible state exists + visible in both themes
- [ ] G2 — hover/active/pressed states exist (crop pass with :hover forced)
- [ ] G3 — loading state per async control (button spinner, skeleton) — no dead click
- [ ] G4 — disabled state visually distinct AND explains itself (tooltip/help)
- [ ] G6 — error state per input (inline message, not just red border)
- [ ] G8 — focus ring collides/overlaps adjacent elements when shown (offset too large in dense rows)

### H. States & data edge cases (per surface — the state-matrix already gates presence; these gate QUALITY)
- [ ] H1 — empty state designed (icon+message+CTA), not blank/plain-text
- [ ] H2 — error state per ErrorState spec (named resource + human copy + retry)
- [ ] H3 — loading state present (skeleton/spinner), cleared on settle — never stuck WITH content/error
- [ ] H4 — single-item vs many-items vs MAX-items (pagination/scroll appears; layout survives 100 rows)
- [ ] H5 — long/hostile content: 200-char titles, no-space strings, emoji, URLs-as-names

### I. Stacking & overlays
- [ ] I2 — toast/overlay obscuring content it refers to (the toast-over-field class)
- [ ] I3 — dropdown/popover clipped by parent overflow

### J. Consistency (cross-surface — needs the side-by-side pass)
- [ ] J1 — same widget, different look across surfaces (buttons, badges, empty states, error patterns)
- [ ] J2 — same action, different label/icon across surfaces
- [ ] J3 — page-header pattern deviates from the majority (title/actions/breadcrumb placement)
- [ ] J4 — spacing rhythm differs between sibling settings pages
- [ ] J5 — component variant inappropriate to context density — heavy/boxed variants (button-look tabs, large controls) in dense/narrow containers (side panels, toolbars) where the quiet variant (underline tabs, compact controls) belongs; variant-selection rules live in DESIGN_SYSTEM.md. (user miss #9a: button-look tabs busy in the chat right panel)

### K. Information architecture & placement
- [ ] K2 — critical action only reachable at a scroll extreme (e.g. save/submit only at the bottom of a long form with no sticky bar)
- [ ] K3 — information placed where its trigger/context isn't (settings that affect X living on page Y; status shown far from the thing it describes)
- [ ] K4 — scroll-state review rule: for scrollable surfaces, review the SCROLLED-MIDDLE state too, not just scroll-top — what chrome/affordances remain visible?

### L. Content-rendering correctness (markdown/rich pipeline)
- [ ] L6 — rendered-quality judgment: equation overflow on mobile, diagram legibility, code block copy affordance present

### M. User affordances & capability completeness (per component: what would a user WANT to do here?)
- [ ] M2 — affordance present but undiscoverable (hidden until hover with no hint, buried in a menu for a primary job)
- [ ] M3 — affordance works but loses user state (toggle resets on rerender, scroll position lost on expand)
- [ ] M4 — jobs-to-be-done review question: "as a user of THIS block/surface, what would I try to do next — and can I?" (quote/reply-to a message, re-run a tool call, open a file from its attachment card, copy a table as CSV/markdown)

## ABSENCE questions — ask on EVERY crop (process-rule 3)

- [ ] What differentiation is MISSING? (roles/kinds that should look different but don't — C7/C8)
- [ ] What affordance is MISSING? (focus/hover/loading/disabled/error state, empty-state CTA — G1-G6/H1)
- [ ] What state is MISSING or stuck? (loading that never clears, error without retry — H2/H3)
- [ ] What decoration is VALUELESS? For each decorative element: what would be lost if removed? (C13)
- [ ] For a SCROLLABLE crop: what chrome/affordance leaves the viewport when scrolled? (K1/K4)

## Per-surface acceptance call-outs

### `settings-citations`
- [ ] C1 (acceptance #4): is any status tag/badge (e.g. "verified") ordered BEFORE the citation key it qualifies? A badge must FOLLOW its label — "vaswani2017attention (verified)", never "(verified) vaswani2017attention".

### `deep-chat-long`
- [ ] C7 (acceptance #6): can you tell a USER message from an ASSISTANT message at a glance? If they share background/alignment/decoration, the roles are indistinguishable.
- [ ] C12/C13 (acceptance #13): the user-message avatar — is it a bare gray placeholder circle with no image/initials? What would be LOST if it were removed? (An avatar that conveys nothing is valueless decoration.)
- [ ] K4: review the SCROLLED-MIDDLE crop too — does conversation context ("In project: …", title, mode) remain visible after scrolling, or does it scroll away?

### `deep-chat-tool-approval`
- [ ] C9/C10 (acceptance #7): the "Tool Approval Required" block — is the icon on its OWN line (split from its label)? Is the icon oversized (>1.6×) relative to the text it labels?

### `deep-chat-right-panel-file`
- [ ] J5 (acceptance #9): the right-panel tab strip — boxed/button-look tabs in a narrow side panel where a quiet UNDERLINE variant belongs?
- [ ] J6 (acceptance #10): the file-viewer action group — do peer icon-only buttons mix variants (Download=outline vs open-sidebar/open-new-tab=ghost)?
- [ ] C11 (acceptance #10): the open-in-new-tab button — does its icon communicate "open in new tab" (ExternalLink), or an ambiguous icon?

### `deep-chat-right-panel-literature`
- [ ] J5: same tab-strip density question as the file panel.

## Captured crops

_(run without `--no-shots` to capture PNGs under `crops/`)_
