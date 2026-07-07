# UI Defect Taxonomy — the comprehensive checklist (v1, 2026-07-06)

Synthesized from: user-caught misses (#1-4), industry design-QA taxonomies (OverlayQA 12 visual-bug classes,
LambdaTest 14 common UI bugs, Eleken/Impeccable design-QA checklists), academic layout-fault research
(ReDeCheck: protrusion/overlap/wrap failure classes), i18n/RTL testing guides.

Detector legend: [G] deterministic DOM-geometry rule (gallery runtime pass — CANNOT be missed once wired)
[V] vision rubric line (component-crop review) · [L] source lint (biome/AST) · [T] tooling (axe, contrast calc)

## A. Spacing & adjacency
- A1 [G] **zero-gap adjacency** — two visible inline siblings (badge/button/chip/link/text) whose boxes touch with no margin/gap. *(user miss #1: "Disconnected Connect")*
- A2 [G] element boxes **overlap** (non-ancestor pairs, both visible, intersection area > threshold)
- A3 [G] element **protrudes** outside its container/viewport (clip without overflow affordance)
- A4 [G] **uneven spacing between like siblings** — list rows/cards in one container with differing gaps
- A5 [V] padding asymmetric where design implies symmetry (card L≠R, modal header vs footer)
- A6 [V] insufficient whitespace between unrelated sections / crowding
- A7 [G] spacing values off the 4px grid (computed margins/paddings/gaps % 4 ≠ 0)
- A10 [G] **interactive control at zero/unusable size** — a form control (input/select/textarea) rendered with zero or near-zero width/height while visible-intent (in an open edit form): the "input disappears" class. *(user miss #16: inline rename form renders vertical, input collapses)*
- A9 [G] **inconsistent internal metrics among peer siblings** — same-kind sibling elements in one group (chips, indicator badges, toolbar buttons, stat pills) whose internal measurements differ: icon box sizes, overall heights, paddings, or font sizes not equal across the peers — makes identical-role elements read as different sizes. **Also covers menu/dropdown/popover ITEM rows** (grouped by their menu container regardless of per-item testid): their LEADING icon boxes must match. Because such menus are interaction-gated, the detector is fed the open state via an interaction recipe (e.g. `open-plus-menu`). *(user miss #15: chat-input "Memory: auto" / "Summary: auto" chips have different icon sizes; + menu "Skills in this chat" BookOpen renders 24px among 16px peers.)*
- A8 [G] **row children not vertically centered** — in row-like containers (tablist/toolbar/chip strip/list row), a child's vertical center deviates >2px from the container's center line (when siblings are centered). *(user miss #9c: right-panel tabs misaligned)*
- A11 [G] **element border clipped by a clipping ancestor** — for every element with a real (opaque, ≥1px) border, walk up to the nearest overflow-clipping ancestor (overflow `hidden`/`auto`/`clip`/`scroll`) per axis and compare each bordered side's border-box edge to that ancestor's INNER clip rect (padding box minus scrollbar gutter). If a bordered side is at/outside that clip edge by within its own border-width, the clip line passes through the border stroke and the border is being cut — flag with the side + measured overshoot. Overshoot is deliberately bounded at ≤ border-width so a genuinely scrolled-OUT element is not flagged (that's normal scrolling, not a defect). Sibling of the G7 focus-ring-clip rule (shares the clipping-ancestor walk). *(deep-chat: tool-call/message cards in the message scroll column whose top/left/right border is eaten by a negative margin or an overflow-hidden card/list container edge.)*

## B. Wrap, reflow & responsive
- B1 [G] **premature wrap** — flex-wrap container wrapped although Σ(children+gaps) ≤ container width. *(user miss #2/#3: Template Assistants/My Assistants/Configured providers header + button)*
- B2 [G] failure-to-wrap — content clipped/protruding where wrap/ellipsis was possible
- B3 [G] horizontal scrollbar/overflow at standard viewports
- B4 [V] wrong stacking order after reflow (actions land above their label; controls disconnected from their row)
- B5 [V] element unusable/cut at 375px (tap targets, buttons half-visible)
- B6 [G] fixed-width element wider than mobile viewport
- B7 [V] desktop dead-gutter / content not using width per the app's standard settings max-width
- B8 [G] mid-word text break (word longer than box without break-word intent)

## C. Order & composition semantics
- C1 [V] **status tag/badge ordered before its label/key** — badges follow the thing they qualify. *(user miss #4: "(verified) vaswani2017attention")*
- C2 [V] icon on the wrong side per app convention (leading icons for nav, trailing for external/chevrons)
- C3 [V] primary/secondary button order inconsistent with the app convention (and OS-idiom per platform)
- C4 [V] orphaned/floating control — a control visually disconnected from what it acts on
- C5 [V] duplicate signals (banner + redundant toast for the same event)
- C6 [V] mixed alignment within one group (some rows left, one centered)
- C7 [G] **indistinguishable roles** — DOM siblings carrying DIFFERENT semantic roles (data-role/testid variants: message-user vs message-assistant; system vs user rows; local vs remote items) whose computed visual signature is IDENTICAL (background, border, alignment/margin pattern, avatar/label presence). The DOM knows the roles; if the pixels don't show them, flag. *(user miss #6: user vs assistant messages look the same)*
- C8 [V] missing differentiation affordance (broader judgment form of C7): any list/thread mixing entity kinds where a reader cannot tell kinds apart at a glance
- C9 [G] **icon/label pair split across lines** — a leaf icon (svg/img) and its text label sibling render on different lines (disjoint y-ranges) although combined width + gap fits the container; catches both flex-wrap and bad block markup. *(user miss #7a: "Tool Approval Required" icon on its own row)*
- C12 [G] **placeholder element shipped** — visual elements in their unfinished/default form: avatar with no image/initials/icon (bare tinted circle), "lorem"/"TODO"/"xxx" copy, default favicons, {unresolved} template braces. Detectable: avatar-shaped elements (rounded-full with background) with NO child content; text scan for placeholder tokens. *(user miss #13a: user-message avatar is a bare gray circle)*
- C13 [V] **valueless decoration** — an element that consumes space but adds no information or affordance (an avatar in a two-party chat where the parties are/should be distinguished by layout; repeated decorative icons; badges that are always the same value). Rubric question: "what would be lost if this element were removed?" — if nothing, flag. *(user miss #13b: the user avatar has no value even if filled)*
- C11 [L/V] **icon-action semantic mismatch** — the icon glyph does not communicate its action. Lintable core: an action-name→expected-icon table (open-in-new-tab→ExternalLink, download→Download, delete→Trash2, copy→Copy, edit→Pencil, settings→Settings, close→X, refresh→RotateCw...); AST lint compares a control's aria-label/tooltip/action text with its imported icon and flags mismatches. Vision rubric covers unlabeled icons. *(user miss #10b: the open-in-new-tab icon doesn't read as open-in-new-tab)*
- C10 [G] **icon disproportionate to adjacent text** — inline/leading icon whose box height exceeds ~1.6x the adjacent text line-height (or is under ~0.6x): oversized/undersized relative to what it labels. *(user miss #7b: the approval icon too big)*

## D. Truncation & content fit
- D1 [G] **text truncated with room** — ellipsized/clipped element whose parent has ≥ text width available
- D2 [V] truncation without ellipsis/title-tooltip affordance (hard clip)
- D3 [V] truncation of the DISTINGUISHING part (IDs/keys truncated so items look identical)
- D4 [G] i18n headroom: fails at +40% text length (pseudo-locale render of key surfaces)
- D5 [V] numeric formatting broken — NaN/undefined/Infinity/raw 0 rendered ("NaN GB")
- D6 [V] widows/orphans in headings; single-word wrapped lines in buttons

## E. Typography
- E1 [G] font-size/weight/line-height off the type scale tokens
- E2 [V] hierarchy inversion (child heading visually heavier than parent)
- E3 [G] line-length beyond readable measure on text-heavy surfaces (>~90ch)
- E4 [V] baseline misalignment of inline icon+text pairs

## F. Color, theme & contrast
- F1 [T] WCAG AA contrast (text + UI components + focus indicators), BOTH themes
- F2 [V] dark-mode-specific invisibility (borders/swatches/dividers vanish; the black-swatch class)
- F3 [L/G] hardcoded colors bypassing tokens (existing lint:colors — keep)
- F4 [V] state colors misused (success/danger/warning semantics)
- F5 [V] disabled-look on enabled controls (the desaturated-primary class)

## G. Interaction states (per interactive component)
- G1 [V/T] focus-visible state exists + visible in both themes
- G2 [V] hover/active/pressed states exist (crop pass with :hover forced)
- G3 [V] loading state per async control (button spinner, skeleton) — no dead click
- G4 [V] disabled state visually distinct AND explains itself (tooltip/help)
- G5 [G] tap-target ≥ 44px on mobile for primary actions
- G6 [V] error state per input (inline message, not just red border)
- G7 [G] **focus ring / painted decoration clipped** — force :focus-visible on each focusable element; compute the ring extent (outline-width + outline-offset, or focus box-shadow spread) as a rect around the border-box; flag if that rect crosses the bounds of any ancestor with overflow hidden/auto/clip, OR crosses the container edge the element is flush against (ring partially cut, e.g. left-aligned input with left ring clipped). Same rule for elevation shadows on cards flush to a clipping ancestor. *(user miss #5)*
- G8 [V] focus ring collides/overlaps adjacent elements when shown (offset too large in dense rows)

## H. States & data edge cases (per surface — the state-matrix already gates presence; these gate QUALITY)
- H1 [V] empty state designed (icon+message+CTA), not blank/plain-text
- H2 [V] error state per ErrorState spec (named resource + human copy + retry)
- H3 [V] loading state present (skeleton/spinner), cleared on settle — never stuck WITH content/error
- H4 [V] single-item vs many-items vs MAX-items (pagination/scroll appears; layout survives 100 rows)
- H5 [V] long/hostile content: 200-char titles, no-space strings, emoji, URLs-as-names
- H6 [G] image aspect/fallback (broken image icon, layout shift on load)
- H7 [G] **empty control renders nothing** — an OPEN picker/select/menu dropdown (`role=listbox`/`role=menu`/`[data-slot=select-content]`) with ZERO selectable option items AND no empty-state hint text ("No models", "No results") — it shows the user literally nothing to select and no affordance to fix it. Interaction-gated: the popup only exists once opened, so it's fed via an `open-…-select` recipe. *(the composer model picker `ullm-model-select` with 0 models configured opens to an empty listbox — no options, no "No models" text, no disabled+configure affordance.)*

## I. Stacking & overlays
- I1 [G] z-index collisions — element unintentionally under another (hit-test at corners)
- I2 [V] toast/overlay obscuring content it refers to (the toast-over-field class)
- I3 [V] dropdown/popover clipped by parent overflow
- I4 [G] modal/sheet within viewport at all sizes; body scroll locked
- I5 [G] **wrong scroll axis on a strip** — a single-row strip element (role=tablist, chip/tab strips) that scrolls VERTICALLY (scrollHeight > clientHeight); strips may overflow horizontally only. *(user miss #9b: right-panel tab list scrolls vertically)*

## J. Consistency (cross-surface — needs the side-by-side pass)
- J1 [V] same widget, different look across surfaces (buttons, badges, empty states, error patterns)
- J2 [V] same action, different label/icon across surfaces
- J3 [V] page-header pattern deviates from the majority (title/actions/breadcrumb placement)
- J4 [V] spacing rhythm differs between sibling settings pages
- J8 [L/G] **raw native scroll where the shared scroll component belongs** — scrollable containers using bare overflow-y auto/scroll classes instead of the kit overlay-scroll wrapper (DivScrollY): native scrollbars on desktop look heavy + inconsistent with the rest of the app. Lint: overflow-(y|x)-(auto|scroll) on raw elements outside the scroll component = flag (allowlist for genuine exceptions e.g. textarea). *(user miss #17: conversation message list uses native scroll on desktop)*
- J7 [G] **same action, different position across components** — recurring controls (expand/collapse, close, copy, fullscreen) must live in the same corner/position of their container everywhere; registry of action-control testids + assert consistent placement side across all containers that have them. *(user miss #12: execute_command block has expand on the RIGHT, file viewer has it on the LEFT)*
- J6 [G] **mixed button variants within a peer action group** — sibling buttons of the same kind (icon-only peers, same size, same role weight) in one action-group container carrying different kit variants (outline vs ghost); intentional primary/secondary hierarchy is allowlisted. Detect via the kit Button's variant class/data-attr on DOM siblings. *(user miss #10a: file-viewer Download=outline vs sidebar/new-tab=ghost)*
- J5 [V] **component variant inappropriate to context density** — heavy/boxed variants (button-look tabs, large controls) in dense/narrow containers (side panels, toolbars) where the quiet variant (underline tabs, compact controls) belongs; variant-selection rules live in DESIGN_SYSTEM.md. *(user miss #9a: button-look tabs busy in the chat right panel)*

## K. Information architecture & placement
- K1 [G/L] **persistent context inside scrollable content** — page/conversation-level metadata or controls (project affiliation, conversation title/status, mode indicators) rendered as a child of the content scroll container instead of pinned chrome: invisible once the user scrolls. Detectable: registry of context-level testids + assert none are descendants of the message/content scroll container; runtime check: scroll the container to bottom and flag context elements that left the viewport. *(user miss #8: "In project: ..." scrolls away in long conversations)*
- K2 [V] critical action only reachable at a scroll extreme (e.g. save/submit only at the bottom of a long form with no sticky bar)
- K3 [V] information placed where its trigger/context isn't (settings that affect X living on page Y; status shown far from the thing it describes)
- K4 [V] scroll-state review rule: for scrollable surfaces, review the SCROLLED-MIDDLE state too, not just scroll-top — what chrome/affordances remain visible?

## L. Content-rendering correctness (markdown/rich pipeline)
- L1 [G] **math fell back to raw source** — markdown containing $..$/$$..$$/\begin{} must produce KaTeX DOM (.katex spans); visible raw "$$", "\begin{aligned}", "\frac" text in rendered output = renderer failure. *(user miss #11a)*
- L2 [G] **mermaid fell back to raw source** — ```mermaid blocks must render an <svg>; a code/pre block containing "graph TD"/"sequenceDiagram" text = failure. *(user miss #11b)*
- L3 [G] **syntax highlighting absent** — a code block with a language class must contain token spans (multi-color Shiki/highlight output); language-tagged block rendered single-color = failure. KNOWN LURKER: Shiki fails under some vite build modes (bundled assets/no wasm) — the detector must run in BOTH dev and preview-build modes. *(user miss #11c)*
- L4 [G] other rich blocks: tables render <table> not pipes; footnotes/links resolve; task-list checkboxes render; nested blockquote structure preserved (heading+list+code inside quote)
- L5 [G] renderer error boundaries: a malformed math/mermaid block must degrade gracefully (styled fallback), not blank the whole message
- L6 [V] rendered-quality judgment: equation overflow on mobile, diagram legibility, code block copy affordance present

## M. User affordances & capability completeness (per component: what would a user WANT to do here?)
- M1 [G] **expected affordance absent** — per the AFFORDANCE_MATRIX (content-type x expected capabilities), a required affordance is missing from the DOM: e.g. code block without copy; mermaid without code/render toggle; html block without sandboxed-iframe render + toggle; image without expand/download; long output without collapse. *(user miss #14: mermaid + html need source/render switching)*
- M2 [V] affordance present but undiscoverable (hidden until hover with no hint, buried in a menu for a primary job)
- M3 [V] affordance works but loses user state (toggle resets on rerender, scroll position lost on expand)
- M4 [V] jobs-to-be-done review question: "as a user of THIS block/surface, what would I try to do next — and can I?" (quote/reply-to a message, re-run a tool call, open a file from its attachment card, copy a table as CSV/markdown)

## N. RTL & directionality readiness
- N1 [L] **physical direction utility in new/changed code** — Tailwind physical-direction classes (`pl-`/`pr-`/`ml-`/`mr-`/`left-`/`right-`/`text-left`/`text-right`) added on this branch, where the logical equivalent (`ps`/`pe`/`ms`/`me`/`start`/`end`/`text-start`/`text-end`) would flip correctly under `dir="rtl"`. ACTIVE source lint, **diff-scoped** (only lines ADDED vs the origin/main merge-base — legacy code isn't punished, and touching a legacy file doesn't fail on its old lines) and AST-scoped to `className` strings (prose/URLs never false-flagged). Gating (exit 1). Genuine physical needs (transform/keyframe anchor, a deliberately LTR-locked scrubber, an icon that must NOT mirror) opt out with an inline `rtl-ok` marker on the line. Detector: `scripts/lint-logical-direction.mjs` (`npm run lint:logical-direction`). *(RTL-readiness by default: keep new components RTL-clean so an eventual i18n/RTL pass is a config flip, not a rewrite.)*
- N2 [T] **`dir="rtl"` render matrix** — render key surfaces under `dir="rtl"` and diff against the LTR baseline for clipping / overlap / mirrored-affordance failures. **DORMANT** until an RTL locale ships (i18n not yet landed) — documented here, deliberately NOT wired.
- N3 [V] **mirrored-crop review** — vision review of RTL crops for icons/affordances that MUST flip (chevrons, back/forward, progress, send) vs. must-NOT (logos, code, media scrubbers, numerals). **DORMANT** until an RTL locale ships — documented here, deliberately NOT wired.

## Process rules
1. **Feedback loop**: every human-caught miss = new taxonomy entry + [G] detector if geometrically expressible, else named [V] rubric line. PRs to this file are cheap; misses recurring is not.
2. **[G] classes gate** (`gate:ui` fails); [V] classes are named lines in the crop-review rubric; [L] classes block commit; [T] run in the runtime pass.
3. Vision rubrics must include ABSENCE questions ("what differentiation/affordance/state is MISSING here?"), not only defect-spotting — absence defects (missing role distinction, missing affordance) are systematically missed by "find what looks wrong" prompts.
4. Vision reviews happen on **component crops at native resolution** (section testids), never full-page-only.
5. Acceptance for the detector build: user misses #1-4 must each be flagged by the system before it ships.
6. **Interaction-gated states are not allowlistable when driveable**: branches reachable by a simple user interaction (click-to-edit, hover-reveal, expand, approval prompts, focus, submit-guard) must get an INTERACTION RECIPE — the gallery entry drives the interaction (`?surface=<slug>&interact=<name>`, `src/dev/gallery/interactions.ts`) before capture — instead of a `coverage-allowlist.json` entry. Allow-list ONLY genuinely undrivable states (drag-mid-flight, OS-native dialogs, error-boundary throws, defensive `length===0` fallbacks). This closes the root-cause gap where ~40 bug-dense interaction states were quarantined as "interaction-gated" allow-list entries and never rendered.
