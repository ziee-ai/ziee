# Affordance Matrix — capability completeness from the USER's perspective

> **New review axis.** Every other UI gate in this repo asks *"is the render
> correct?"* (geometry, contrast, no crashes — see `gate:ui` /
> `runtime-health.mjs`). This document adds the orthogonal question: **"can the
> user *do* what they'd naturally want with each thing on screen?"**
>
> A code block that renders beautifully but offers no **copy** button is a
> *correct render* and a *capability failure*. The affordance matrix enumerates,
> per content/component type, the actions a user expects (benchmarked against
> ChatGPT and Claude.ai), grades each **REQUIRED / NICE / N-A**, audits ziee
> against it, and ships a deterministic detector so a REQUIRED affordance can
> never silently regress.

- **Matrix size:** 15 content/component types × 20 affordances = **300 cells.**
- **Detector:** `scripts/affordance-audit.mjs` (M1 presence checks over the
  gallery's rich-conversation deep-states, allowlist-gated).
- **Sources benchmarked:** ChatGPT, Claude.ai, GitHub-flavored markdown
  rendering (Mermaid-in-iframe), and general chat-UX conventions.

---

## 1. Legend

| Mark | Meaning |
|---|---|
| **R** | **REQUIRED** — a user will predictably want this; its absence is a real capability gap. Every R cell carries a one-line user story in §4. |
| **N** | **NICE** — expected in best-in-class UIs, but the flow survives without it. |
| **–** | **N-A** — the affordance is meaningless for this type. |

Content/component types (rows) and affordances (columns) are defined in §2–§3.
The matrix (§3) states **expectations only** — what *should* exist. The audit
(§5) states **reality** — what ziee ships today. Keep the two separate: the
matrix is the spec; the gap table is the delta.

---

## 2. Content & component types (rows)

| # | Type | Where it renders in ziee |
|---|---|---|
| 1 | **Code block** (```` ```lang ````) | Streamdown → `[data-streamdown="code-block"]` |
| 2 | **Mermaid block** (```` ```mermaid ````) | Streamdown `@streamdown/mermaid` → `[data-streamdown="mermaid-block"]` |
| 3 | **HTML block** (```` ```html ````) | Streamdown code block (raw HTML is **sanitized**, never live-rendered) |
| 4 | **Math** (`$…$`, `$$…$$`) | Streamdown KaTeX |
| 5 | **Table** (GFM pipe table) | `MarkdownTable` override → `[data-streamdown="table-wrapper"]` |
| 6 | **Image** (markdown `![]()`) | `img` override in `useStreamdownComponents` |
| 7 | **File attachment card** (tool-result / message file) | `InlineFilePreview` → `[data-testid="inline-file-preview"]` |
| 8 | **Tool-call block** | `McpToolCallUI` → `[data-testid^="mcp-toolcall-card-"]` |
| 9 | **User message** | `MessageActions` (user branch) |
| 10 | **Assistant message** | `MessageActions` (assistant branch) |
| 11 | **Long output** (any oversized block) | height-capped scroll regions |
| 12 | **Link** (inline `[text](url)`) | `a` override in `useStreamdownComponents` |
| 13 | **Right-panel file viewer** | `ChatRightPanel` + `viewers/*` |
| 14 | **Conversation** (thread, app-level) | `ConversationPage` / `MessageList` |
| 15 | **Conversation list** (app-level) | `ConversationList` |

## 3. The matrix (expectations)

Column keys: **Copy** copy primary content/source · **Dl** download · **Rndr**
live render · **Tgl** source⇄render toggle · **Wrap** soft-wrap toggle · **Ln#**
line numbers · **Lang** language label · **Exp** expand/collapse long · **Zm**
zoom/fullscreen/lightbox · **NwTb** open-in-new-tab · **Panel** open-in-side-panel
· **EdBr** edit-and-branch · **Rgn** regenerate · **Brnch** branch nav · **Qt**
quote/reply · **Alt** alt/accessible label · **ExtL** external-open indicator ·
**Srch** search · **JmpB** jump-to-bottom · **Bulk** bulk-select.

| Type \ Affordance | Copy | Dl | Rndr | Tgl | Wrap | Ln# | Lang | Exp | Zm | NwTb | Panel | EdBr | Rgn | Brnch | Qt | Alt | ExtL | Srch | JmpB | Bulk |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 Code block | **R** | N | – | – | N | N | **R** | N | – | – | – | – | – | – | – | – | – | – | – | – |
| 2 Mermaid block | **R** | **R** | **R** | **R** | – | – | – | – | N | N | – | – | – | – | – | N | – | – | – | – |
| 3 HTML block | **R** | N | **R** | **R** | – | – | **R** | – | – | N | – | – | – | – | – | – | – | – | – | – |
| 4 Math | **R** | – | **R** | – | – | – | – | – | – | – | – | – | – | – | – | N | – | – | – | – |
| 5 Table | **R** | **R** | – | – | – | – | – | N | **R** | – | – | – | – | – | – | – | – | – | – | – |
| 6 Image (md) | N | N | **R** | – | – | – | – | – | **R** | N | – | – | – | – | – | **R** | – | – | – | – |
| 7 Attachment card | – | **R** | N | – | – | – | – | **R** | N | **R** | **R** | – | – | – | – | **R** | – | – | – | – |
| 8 Tool-call block | **R** | – | – | – | – | – | – | **R** | – | – | – | – | N | – | – | – | – | – | – | – |
| 9 User message | **R** | – | – | – | – | – | – | – | – | – | – | **R** | – | **R** | N | – | – | – | – | – |
| 10 Assistant message | **R** | – | – | – | – | – | – | – | – | – | – | – | **R** | **R** | N | – | – | – | – | – |
| 11 Long output | – | – | – | – | – | – | – | **R** | – | – | – | – | – | – | – | – | – | – | – | – |
| 12 Link | N | – | – | – | – | – | – | – | – | **R** | – | – | – | – | – | – | N | – | – | – |
| 13 Right-panel viewer | N | **R** | **R** | – | – | – | – | **R** | **R** | **R** | – | – | – | – | – | **R** | – | – | – | – |
| 14 Conversation | **R** | N | – | – | – | – | – | – | – | – | – | – | – | **R** | – | – | – | N | **R** | – |
| 15 Conversation list | – | – | – | – | – | – | – | – | – | – | – | – | – | – | – | – | – | **R** | N | N |

**R count = 40 · N count = 24 · N-A = 236.**

---

## 4. REQUIRED-cell user stories

Each `R` above, justified. (Grouped by type; the affordance name matches the
column key.)

**1 Code block** — *Copy:* "I want to paste this snippet into my editor without
hand-selecting it." *Lang:* "I want to know at a glance whether this is Python or
Rust before I trust it."

**2 Mermaid block** — *Rndr:* "Show me the diagram, not the source." *Tgl:* "Let
me flip to the source to tweak or verify it." *Copy:* "Let me copy the diagram
source into my own `.mmd` / doc." *Dl:* "Let me save the diagram as an image for
a slide."

**3 HTML block** — *Rndr:* "The model wrote me a landing-page / SVG / chart — let
me see it rendered, safely sandboxed." *Tgl:* "Let me flip between the preview
and the source." *Copy/Lang:* "Let me copy the HTML and know it's HTML."

**4 Math** — *Rndr:* "Render the equation, don't show me raw `\int`." *Copy:*
"Let me copy the TeX source into my paper."

**5 Table** — *Copy:* "Let me copy this as markdown/CSV into a doc or sheet."
*Dl:* "Let me download it as CSV." *Zm:* "This table is wide — let me expand it
to full screen to actually read it."

**6 Image (md)** — *Rndr:* "Show the image inline." *Zm:* "Let me click to
enlarge / lightbox a small inline image." *Alt:* "Screen-reader + broken-image
users need the alt text."

**7 Attachment card** — *Dl:* "Let me download the file the tool produced." *Exp:*
"Let me expand/collapse the preview so it doesn't dominate the thread." *NwTb:*
"Let me open it full-size in a new tab." *Panel:* "Let me pin it in the side
panel while I keep reading." *Alt:* "Icon + filename must be labeled for a11y."

**8 Tool-call block** — *Copy:* "Let me copy the JSON result to inspect or reuse
it." *Exp:* "Let me expand the args/result — collapsed by default keeps the
thread readable."

**9 User message** — *Copy:* "Let me copy what I said." *EdBr:* "Let me edit my
prompt and re-run it on a new branch." *Brnch:* "Let me navigate the branches my
edits created."

**10 Assistant message** — *Copy:* "Let me copy the whole reply." *Rgn:* "That
answer was off — regenerate it." *Brnch:* "Let me compare regenerated branches."

**11 Long output** — *Exp:* "A 400-line block shouldn't force me to scroll past
it forever — cap it and let me expand."

**12 Link** — *NwTb:* "An external link should open in a new tab so I don't lose
my conversation."

**13 Right-panel viewer** — *Rndr/Exp/Zm/Dl/NwTb/Alt:* "The panel is where I
actually read a file — I need to render it, expand it, zoom it, download it, pop
it out, and have it labeled."

**14 Conversation** — *Copy:* "Let me copy the whole exchange." *Brnch:* "Let me
move between conversation branches." *JmpB:* "I scrolled up in a long thread — one
click back to the latest message."

**15 Conversation list** — *Srch:* "Let me find a past conversation by keyword."

---

## 5. Audit — ziee today vs. the matrix (gap table)

Verified by reading the components + inspecting the rendered gallery
deep-states (`deep-chat-streaming`, `deep-chat-long`,
`deep-chat-mcp-toolcall-completed`) and the Streamdown 2.5.0 DOM.

### 5a. Well-covered (no gap — these are the regression guards)

| Type | Affordances present |
|---|---|
| Code block | copy, download, wrap toggle, line numbers, language label *(all Streamdown-native)* |
| Table | copy (md/CSV), download, fullscreen *(`MarkdownTable` + Streamdown dropdowns)* |
| Attachment card | expand/collapse, open-new-tab, open-side-panel, viewer download, a11y label |
| Tool-call block | expand/collapse (args + result), progress |
| User message | copy, edit-and-branch, branch nav |
| Assistant message | copy, regenerate, branch nav |
| Link | open-in-new-tab (`target=_blank rel=noreferrer`) |
| Math | live KaTeX render |
| Image (md) | inline render, alt passthrough |
| Right-panel viewer | render, expand, zoom, download, open-new-tab (per-viewer chrome) |
| Conversation list | search |

### 5b. Gaps (component | missing REQUIRED/NICE affordance | severity)

| # | Component | Missing affordance | Grade | Severity |
|---|---|---|---|---|
| G1 | **Mermaid block** | source⇄render **toggle** | R | **HIGH** |
| G2 | **Mermaid block** | **copy source** | R | **HIGH** |
| G3 | **HTML block** | sandboxed-iframe **live render** + toggle | R | **HIGH** |
| G4 | **Tool-call block** | **copy result** | R | **MED** |
| G5 | **Image (md)** | **lightbox / expand** | R | **MED** |
| G6 | **Image (md)** | alt is passed but not surfaced on broken/hover | R | **LOW** |
| G7 | **Conversation** | **jump-to-bottom** button | R | **MED** |
| G8 | **Mermaid block** | zoom / pan | N | LOW |
| G9 | **Math** | copy TeX source | N | LOW |
| G10 | **Image (md)** | download | N | LOW |
| G11 | **Conversation** | in-conversation (in-thread) search | N | LOW |
| G12 | **User/assistant message** | quote/reply (select-to-quote) | N | LOW |
| G13 | **Link** | external-open indicator icon | N | LOW |
| G14 | **Code block** | explicit collapse for very-long (height-capped only) | N | LOW |
| G15 | **Conversation list** | bulk select / multi-delete | N | LOW |

**Severity counts:** HIGH = 3 · MED = 3 · LOW (incl. NICE) = 9. **Total = 15.**

> **Note on G1–G3:** Streamdown *renders* mermaid and gives it a download
> control (`mermaid-block-actions`), but ships **no source toggle and no
> copy-source**; and it **sanitizes** raw HTML rather than rendering it. These
> are the two user-named backlog features (§7 a/b) plus the copy-source rider.

---

## 6. Detector — `scripts/affordance-audit.mjs`

A deterministic **M1 presence pass** — the affordance analog of the geometry /
runtime-health audit. It drives the gallery's rich-conversation deep-states with
Playwright and, for every rendered content-type container, asserts its
**REQUIRED deterministic controls** are present in the DOM:

| Rule | Selector asserted present under each container |
|---|---|
| code-copy | `[data-streamdown="code-block"]` ⊃ `[data-streamdown="code-block-copy-button"]` |
| table-copy | `[data-streamdown="table-wrapper"]` ⊃ `TableCopyDropdown` control |
| table-fullscreen | `[data-streamdown="table-wrapper"]` ⊃ `[data-testid="markdown-table-fullscreen-btn"]` |
| mermaid-toggle | `[data-streamdown="mermaid-block"]` ⊃ a source-toggle control ← **allowlisted gap (G1)** |
| toolcall-expand | `[data-testid^="mcp-toolcall-card-"]` ⊃ `[data-testid^="mcp-toolcall-details-btn-"]` |
| message-copy | `[data-testid="chat-message"]` ⊃ `[data-testid="chat-message-copy-btn"]` |
| attachment-newtab | `[data-testid="inline-file-preview"]` ⊃ `[data-testid="inline-file-preview-open"]` |

**Allowlist gating** (`scripts/affordance-audit-allowlist.json`): a rule listed
there reports as an **ALLOWED** gap (documented, non-gating) instead of failing —
so the two known backlog gaps (mermaid toggle, HTML render) keep the detector
green while remaining tracked. **Any NEW missing control** — e.g. someone deletes
the code-copy button — is **not** allowlisted and **fails** the run
(non-zero exit). This makes the detector simultaneously a *regression guard* for
shipped affordances and a *tracker* for the backlog.

```bash
# Run against a running gallery (same contract as runtime-health.mjs):
node scripts/affordance-audit.mjs                 # gates on non-allowlisted misses
node scripts/affordance-audit.mjs --report-only   # never exits non-zero
# Output: AFFORDANCE_FINDINGS.{jsonl,md} next to the gallery findings.
```

---

## 7. Backlog (implement AFTER this matrix lands, as feature-lifecycle work)

Prioritized. **(a)** and **(b)** are the two user-named, confirmed gaps and are
**NOT** implemented on this branch — this branch is matrix + audit + detector
only.

| Rank | Feature | Addresses | Grade |
|---|---|---|---|
| 1 | **(a) Mermaid code⇄render toggle** (+ copy source) | G1, G2 | HIGH |
| 2 | **(b) HTML block sandboxed-iframe render + toggle** | G3 | HIGH |
| 3 | Tool-call **copy result** button | G4 | MED |
| 4 | Markdown-image **lightbox / expand** | G5 | MED |
| 5 | Conversation **jump-to-bottom** button | G7 | MED |
| 6 | Mermaid **zoom / pan** | G8 | LOW/NICE |
| 7 | Math **copy-TeX** control | G9 | NICE |
| 8 | Markdown-image **download** | G10 | NICE |
| 9 | In-conversation (in-thread) **search** | G11 | NICE |
| 10 | Message **quote/reply** | G12 | NICE |

Each backlog item, when built, adds its detector rule (and removes its allowlist
entry) so the affordance-audit converts from *tracking* the gap to *guarding* the
new affordance.
