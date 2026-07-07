# Geometry findings (GENERATED)

> `node scripts/gallery-geometry-audit.mjs` (Layer 1 — see `docs/DEFECT_TAXONOMY.md`). Deterministic DOM-geometry rules over every gallery surface × state × viewport. Each row cites surface, viewport, taxonomy class, selector, and measured numbers.

## Totals

| Severity | Count |
|---|---|
| 🔴 HIGH (gating) | 0 |
| 🔵 HIGH (allow-listed) | 15 |
| 🟡 MEDIUM | 1459 |
| ⚪ LOW | 3812 |
| **Total** | **5286** |

## By taxonomy class

| Class | HIGH | MEDIUM | LOW | allow-listed |
|---|---|---|---|---|
| A1 | 3 | 0 | 0 | 3 |
| A10 | 0 | 3 | 0 | 0 |
| A11 | 0 | 6 | 0 | 0 |
| A12 | 0 | 0 | 197 | 0 |
| A13 | 0 | 44 | 0 | 0 |
| A14 | 0 | 0 | 873 | 0 |
| A2 | 0 | 85 | 0 | 0 |
| A3 | 0 | 181 | 0 | 0 |
| A4 | 0 | 0 | 20 | 0 |
| A5 | 0 | 78 | 0 | 0 |
| A7 | 0 | 0 | 379 | 0 |
| A8 | 0 | 6 | 0 | 0 |
| A9 | 0 | 7 | 0 | 0 |
| B1 | 9 | 20 | 0 | 9 |
| B8 | 0 | 0 | 33 | 0 |
| C1 | 0 | 6 | 0 | 0 |
| C10 | 0 | 15 | 0 | 0 |
| C12 | 0 | 3 | 48 | 0 |
| C7 | 0 | 3 | 0 | 0 |
| C9 | 0 | 3 | 0 | 0 |
| D1 | 0 | 114 | 0 | 0 |
| G5 | 0 | 286 | 2262 | 0 |
| G7 | 0 | 128 | 0 | 0 |
| G9 | 0 | 9 | 0 | 0 |
| H7 | 0 | 51 | 0 | 0 |
| I1 | 0 | 159 | 0 | 0 |
| I4 | 0 | 6 | 0 | 0 |
| I5 | 0 | 3 | 0 | 0 |
| J6 | 0 | 10 | 0 | 0 |
| J7 | 0 | 188 | 0 | 0 |
| K1 | 0 | 3 | 0 | 0 |
| L1 | 3 | 0 | 0 | 3 |
| L2 | 0 | 3 | 0 | 0 |
| L3 | 0 | 39 | 0 | 0 |

## Gating HIGH findings (0)

_None — geometry is clean of un-allow-listed HIGH findings._

## MEDIUM findings (1459)

### A10 (3)

- 🟡 `seeded-defect-repro` [desktop] `[data-testid="repro-a10-input"]` — form control <input> rendered 2×32px (near-zero width) while visible-intent — the "input disappears" class
- 🟡 `seeded-defect-repro` [mobile] `[data-testid="repro-a10-input"]` — form control <input> rendered 2×32px (near-zero width) while visible-intent — the "input disappears" class
- 🟡 `seeded-defect-repro` [tablet] `[data-testid="repro-a10-input"]` — form control <input> rendered 2×32px (near-zero width) while visible-intent — the "input disappears" class

### A11 (6)

- 🟡 `seeded-defect-repro` [desktop] `[data-testid="repro-a11-card"]` — right border (1px) clipped by overflow-x ancestor [data-testid="repro-a11-clip"] — cut 60.0px
- 🟡 `seeded-defect-repro` [desktop] `[data-testid="repro-a11-card"]` — bordered element's right border clipped by overflow-x ancestor [data-testid="repro-a11-clip"] (60px past the clip edge)
- 🟡 `seeded-defect-repro` [mobile] `[data-testid="repro-a11-card"]` — right border (1px) clipped by overflow-x ancestor [data-testid="repro-a11-clip"] — cut 60.0px
- 🟡 `seeded-defect-repro` [mobile] `[data-testid="repro-a11-card"]` — bordered element's right border clipped by overflow-x ancestor [data-testid="repro-a11-clip"] (60px past the clip edge)
- 🟡 `seeded-defect-repro` [tablet] `[data-testid="repro-a11-card"]` — right border (1px) clipped by overflow-x ancestor [data-testid="repro-a11-clip"] — cut 60.0px
- 🟡 `seeded-defect-repro` [tablet] `[data-testid="repro-a11-card"]` — bordered element's right border clipped by overflow-x ancestor [data-testid="repro-a11-clip"] (60px past the clip edge)

### A13 (44)

- 🟡 `deep-chat-long` [desktop] `[data-testid="file-card"]` — left-aligned block inside a right-aligned message [data-testid="chat-message"]: its right edge is 614px (84% of the message width) from the message's right edge — it detaches from the bubble instead of following its right-alignment
- 🟡 `deep-chat-long` [desktop] `[data-testid="file-card"]` — left-aligned block inside a right-aligned message [data-testid="chat-message"]: its right edge is 614px (84% of the message width) from the message's right edge — it detaches from the bubble instead of following its right-alignment
- 🟡 `deep-chat-long` [desktop] `[data-testid="file-card"]` — left-aligned block inside a right-aligned message [data-testid="chat-message"]: its right edge is 614px (84% of the message width) from the message's right edge — it detaches from the bubble instead of following its right-alignment
- 🟡 `deep-chat-long` [mobile] `[data-testid="file-card"]` — left-aligned block inside a right-aligned message [data-testid="chat-message"]: its right edge is 113px (48% of the message width) from the message's right edge — it detaches from the bubble instead of following its right-alignment
- 🟡 `deep-chat-long` [mobile] `[data-testid="file-card"]` — left-aligned block inside a right-aligned message [data-testid="chat-message"]: its right edge is 113px (48% of the message width) from the message's right edge — it detaches from the bubble instead of following its right-alignment
- 🟡 `deep-chat-long` [mobile] `[data-testid="file-card"]` — left-aligned block inside a right-aligned message [data-testid="chat-message"]: its right edge is 113px (48% of the message width) from the message's right edge — it detaches from the bubble instead of following its right-alignment
- 🟡 `deep-chat-long` [mobile] `[data-testid="file-card"]` — left-aligned block inside a right-aligned message [data-testid="chat-message"]: its right edge is 113px (48% of the message width) from the message's right edge — it detaches from the bubble instead of following its right-alignment
- 🟡 `deep-chat-long` [mobile] `[data-testid="file-card"]` — left-aligned block inside a right-aligned message [data-testid="chat-message"]: its right edge is 113px (48% of the message width) from the message's right edge — it detaches from the bubble instead of following its right-alignment
- 🟡 `deep-chat-long` [mobile] `[data-testid="file-card"]` — left-aligned block inside a right-aligned message [data-testid="chat-message"]: its right edge is 113px (48% of the message width) from the message's right edge — it detaches from the bubble instead of following its right-alignment
- 🟡 `deep-chat-long` [tablet] `[data-testid="file-card"]` — left-aligned block inside a right-aligned message [data-testid="chat-message"]: its right edge is 434px (78% of the message width) from the message's right edge — it detaches from the bubble instead of following its right-alignment
- 🟡 `deep-chat-long` [tablet] `[data-testid="file-card"]` — left-aligned block inside a right-aligned message [data-testid="chat-message"]: its right edge is 434px (78% of the message width) from the message's right edge — it detaches from the bubble instead of following its right-alignment
- 🟡 `deep-chat-long` [tablet] `[data-testid="file-card"]` — left-aligned block inside a right-aligned message [data-testid="chat-message"]: its right edge is 434px (78% of the message width) from the message's right edge — it detaches from the bubble instead of following its right-alignment
- 🟡 `deep-chat-no-models` [desktop] `[data-testid="file-card"]` — left-aligned block inside a right-aligned message [data-testid="chat-message"]: its right edge is 614px (84% of the message width) from the message's right edge — it detaches from the bubble instead of following its right-alignment
- 🟡 `deep-chat-no-models` [desktop] `[data-testid="file-card"]` — left-aligned block inside a right-aligned message [data-testid="chat-message"]: its right edge is 614px (84% of the message width) from the message's right edge — it detaches from the bubble instead of following its right-alignment
- 🟡 `deep-chat-no-models` [mobile] `[data-testid="file-card"]` — left-aligned block inside a right-aligned message [data-testid="chat-message"]: its right edge is 113px (48% of the message width) from the message's right edge — it detaches from the bubble instead of following its right-alignment
- 🟡 `deep-chat-no-models` [mobile] `[data-testid="file-card"]` — left-aligned block inside a right-aligned message [data-testid="chat-message"]: its right edge is 113px (48% of the message width) from the message's right edge — it detaches from the bubble instead of following its right-alignment
- 🟡 `deep-chat-no-models` [mobile] `[data-testid="file-card"]` — left-aligned block inside a right-aligned message [data-testid="chat-message"]: its right edge is 113px (48% of the message width) from the message's right edge — it detaches from the bubble instead of following its right-alignment
- 🟡 `deep-chat-no-models` [mobile] `[data-testid="file-card"]` — left-aligned block inside a right-aligned message [data-testid="chat-message"]: its right edge is 113px (48% of the message width) from the message's right edge — it detaches from the bubble instead of following its right-alignment
- 🟡 `deep-chat-no-models` [tablet] `[data-testid="file-card"]` — left-aligned block inside a right-aligned message [data-testid="chat-message"]: its right edge is 434px (78% of the message width) from the message's right edge — it detaches from the bubble instead of following its right-alignment
- 🟡 `deep-chat-no-models` [tablet] `[data-testid="file-card"]` — left-aligned block inside a right-aligned message [data-testid="chat-message"]: its right edge is 434px (78% of the message width) from the message's right edge — it detaches from the bubble instead of following its right-alignment
- 🟡 `deep-chat-right-panel-file` [desktop] `[data-testid="file-card"]` — left-aligned block inside a right-aligned message [data-testid="chat-message"]: its right edge is 495px (81% of the message width) from the message's right edge — it detaches from the bubble instead of following its right-alignment
- 🟡 `deep-chat-right-panel-file` [mobile] `[data-testid="file-card"]` — left-aligned block inside a right-aligned message [data-testid="chat-message"]: its right edge is 113px (48% of the message width) from the message's right edge — it detaches from the bubble instead of following its right-alignment
- 🟡 `deep-chat-right-panel-file` [mobile] `[data-testid="file-card"]` — left-aligned block inside a right-aligned message [data-testid="chat-message"]: its right edge is 113px (48% of the message width) from the message's right edge — it detaches from the bubble instead of following its right-alignment
- 🟡 `deep-chat-right-panel-literature` [desktop] `[data-testid="file-card"]` — left-aligned block inside a right-aligned message [data-testid="chat-message"]: its right edge is 495px (81% of the message width) from the message's right edge — it detaches from the bubble instead of following its right-alignment
- 🟡 `deep-chat-right-panel-literature` [mobile] `[data-testid="file-card"]` — left-aligned block inside a right-aligned message [data-testid="chat-message"]: its right edge is 113px (48% of the message width) from the message's right edge — it detaches from the bubble instead of following its right-alignment
- 🟡 `deep-chat-right-panel-literature` [mobile] `[data-testid="file-card"]` — left-aligned block inside a right-aligned message [data-testid="chat-message"]: its right edge is 113px (48% of the message width) from the message's right edge — it detaches from the bubble instead of following its right-alignment
- 🟡 `deep-chat-right-panel-multi` [desktop] `[data-testid="file-card"]` — left-aligned block inside a right-aligned message [data-testid="chat-message"]: its right edge is 495px (81% of the message width) from the message's right edge — it detaches from the bubble instead of following its right-alignment
- 🟡 `deep-chat-right-panel-multi` [mobile] `[data-testid="file-card"]` — left-aligned block inside a right-aligned message [data-testid="chat-message"]: its right edge is 113px (48% of the message width) from the message's right edge — it detaches from the bubble instead of following its right-alignment
- 🟡 `deep-chat-right-panel-multi` [mobile] `[data-testid="file-card"]` — left-aligned block inside a right-aligned message [data-testid="chat-message"]: its right edge is 113px (48% of the message width) from the message's right edge — it detaches from the bubble instead of following its right-alignment
- 🟡 `deep-chat-streaming` [desktop] `[data-testid="file-card"]` — left-aligned block inside a right-aligned message [data-testid="chat-message"]: its right edge is 614px (84% of the message width) from the message's right edge — it detaches from the bubble instead of following its right-alignment
- … +14 more (see JSONL)

### A2 (85)

- 🟡 `deep-chat-long` [mobile] `code.rounded.bg-muted ∩ a.wrap-anywhere.font-medium` — sibling boxes overlap by 538px² (100% of smaller)
- 🟡 `deep-chat-long` [mobile] `code.rounded.bg-muted ∩ a.wrap-anywhere.font-medium` — sibling boxes overlap by 538px² (100% of smaller)
- 🟡 `deep-chat-long` [mobile] `code.rounded.bg-muted ∩ a.wrap-anywhere.font-medium` — sibling boxes overlap by 538px² (100% of smaller)
- 🟡 `deep-chat-long` [mobile] `del ∩ code.rounded.bg-muted` — sibling boxes overlap by 1927px² (95% of smaller)
- 🟡 `deep-chat-long` [mobile] `del ∩ code.rounded.bg-muted` — sibling boxes overlap by 1927px² (95% of smaller)
- 🟡 `deep-chat-long` [mobile] `del ∩ code.rounded.bg-muted` — sibling boxes overlap by 1927px² (95% of smaller)
- 🟡 `deep-chat-long` [mobile] `em ∩ code.rounded.bg-muted` — sibling boxes overlap by 4108px² (41% of smaller)
- 🟡 `deep-chat-long` [mobile] `em ∩ code.rounded.bg-muted` — sibling boxes overlap by 4108px² (41% of smaller)
- 🟡 `deep-chat-long` [mobile] `em ∩ code.rounded.bg-muted` — sibling boxes overlap by 4108px² (41% of smaller)
- 🟡 `deep-chat-long` [mobile] `em ∩ del` — sibling boxes overlap by 2034px² (100% of smaller)
- 🟡 `deep-chat-long` [mobile] `em ∩ del` — sibling boxes overlap by 2034px² (100% of smaller)
- 🟡 `deep-chat-long` [mobile] `em ∩ del` — sibling boxes overlap by 2034px² (100% of smaller)
- 🟡 `deep-chat-long` [mobile] `em ∩ em` — sibling boxes overlap by 726px² (100% of smaller)
- 🟡 `deep-chat-long` [mobile] `em ∩ em` — sibling boxes overlap by 726px² (100% of smaller)
- 🟡 `deep-chat-long` [mobile] `em ∩ em` — sibling boxes overlap by 726px² (100% of smaller)
- 🟡 `deep-chat-long` [mobile] `span.font-semibold ∩ em` — sibling boxes overlap by 748px² (100% of smaller)
- 🟡 `deep-chat-long` [mobile] `span.font-semibold ∩ em` — sibling boxes overlap by 748px² (100% of smaller)
- 🟡 `deep-chat-long` [mobile] `span.font-semibold ∩ em` — sibling boxes overlap by 748px² (100% of smaller)
- 🟡 `deep-chat-no-models` [mobile] `code.rounded.bg-muted ∩ a.wrap-anywhere.font-medium` — sibling boxes overlap by 538px² (100% of smaller)
- 🟡 `deep-chat-no-models` [mobile] `code.rounded.bg-muted ∩ a.wrap-anywhere.font-medium` — sibling boxes overlap by 538px² (100% of smaller)
- 🟡 `deep-chat-no-models` [mobile] `del ∩ code.rounded.bg-muted` — sibling boxes overlap by 1927px² (95% of smaller)
- 🟡 `deep-chat-no-models` [mobile] `del ∩ code.rounded.bg-muted` — sibling boxes overlap by 1927px² (95% of smaller)
- 🟡 `deep-chat-no-models` [mobile] `em ∩ code.rounded.bg-muted` — sibling boxes overlap by 4108px² (41% of smaller)
- 🟡 `deep-chat-no-models` [mobile] `em ∩ code.rounded.bg-muted` — sibling boxes overlap by 4108px² (41% of smaller)
- 🟡 `deep-chat-no-models` [mobile] `em ∩ del` — sibling boxes overlap by 2034px² (100% of smaller)
- 🟡 `deep-chat-no-models` [mobile] `em ∩ del` — sibling boxes overlap by 2034px² (100% of smaller)
- 🟡 `deep-chat-no-models` [mobile] `em ∩ em` — sibling boxes overlap by 726px² (100% of smaller)
- 🟡 `deep-chat-no-models` [mobile] `em ∩ em` — sibling boxes overlap by 726px² (100% of smaller)
- 🟡 `deep-chat-no-models` [mobile] `span.font-semibold ∩ em` — sibling boxes overlap by 748px² (100% of smaller)
- 🟡 `deep-chat-no-models` [mobile] `span.font-semibold ∩ em` — sibling boxes overlap by 748px² (100% of smaller)
- … +55 more (see JSONL)

### A3 (181)

- 🟡 `chat` [mobile] `div.flex.items-center` — protrudes 10px past parent div.flex.justify-between (no overflow clip)
- 🟡 `deep-chat-long` [mobile] `[data-testid="inline-file-preview-chevron"]` — protrudes 62px past parent div.flex.items-center (no overflow clip)
- 🟡 `deep-chat-long` [mobile] `[data-testid="inline-file-preview-chevron"]` — protrudes 62px past parent div.flex.items-center (no overflow clip)
- 🟡 `deep-chat-long` [mobile] `[data-testid="inline-file-preview-chevron"]` — protrudes 44px past parent div.flex.items-center (no overflow clip)
- 🟡 `deep-chat-long` [mobile] `[data-testid="inline-file-preview-chevron"]` — protrudes 62px past parent div.flex.items-center (no overflow clip)
- 🟡 `deep-chat-long` [mobile] `[data-testid="inline-file-preview-chevron"]` — protrudes 62px past parent div.flex.items-center (no overflow clip)
- 🟡 `deep-chat-long` [mobile] `[data-testid="inline-file-preview-chevron"]` — protrudes 62px past parent div.flex.items-center (no overflow clip)
- 🟡 `deep-chat-long` [mobile] `[data-testid="inline-file-preview-chevron"]` — protrudes 44px past parent div.flex.items-center (no overflow clip)
- 🟡 `deep-chat-long` [mobile] `[data-testid="inline-file-preview-chevron"]` — protrudes 62px past parent div.flex.items-center (no overflow clip)
- 🟡 `deep-chat-long` [mobile] `[data-testid="inline-file-preview-chevron"]` — protrudes 62px past parent div.flex.items-center (no overflow clip)
- 🟡 `deep-chat-long` [mobile] `[data-testid="inline-file-preview-chevron"]` — protrudes 62px past parent div.flex.items-center (no overflow clip)
- 🟡 `deep-chat-long` [mobile] `[data-testid="inline-file-preview-chevron"]` — protrudes 44px past parent div.flex.items-center (no overflow clip)
- 🟡 `deep-chat-long` [mobile] `[data-testid="inline-file-preview-chevron"]` — protrudes 62px past parent div.flex.items-center (no overflow clip)
- 🟡 `deep-chat-long` [mobile] `[data-testid="inline-file-preview-open"]` — protrudes 16px past parent div.flex.items-center (no overflow clip)
- 🟡 `deep-chat-long` [mobile] `[data-testid="inline-file-preview-open"]` — protrudes 16px past parent div.flex.items-center (no overflow clip)
- 🟡 `deep-chat-long` [mobile] `[data-testid="inline-file-preview-open"]` — protrudes 14px past parent div.flex.items-center (no overflow clip)
- 🟡 `deep-chat-long` [mobile] `[data-testid="inline-file-preview-open"]` — protrudes 16px past parent div.flex.items-center (no overflow clip)
- 🟡 `deep-chat-long` [mobile] `[data-testid="inline-file-preview-open"]` — protrudes 16px past parent div.flex.items-center (no overflow clip)
- 🟡 `deep-chat-long` [mobile] `[data-testid="inline-file-preview-open"]` — protrudes 16px past parent div.flex.items-center (no overflow clip)
- 🟡 `deep-chat-long` [mobile] `[data-testid="inline-file-preview-open"]` — protrudes 14px past parent div.flex.items-center (no overflow clip)
- 🟡 `deep-chat-long` [mobile] `[data-testid="inline-file-preview-open"]` — protrudes 16px past parent div.flex.items-center (no overflow clip)
- 🟡 `deep-chat-long` [mobile] `[data-testid="inline-file-preview-open"]` — protrudes 16px past parent div.flex.items-center (no overflow clip)
- 🟡 `deep-chat-long` [mobile] `[data-testid="inline-file-preview-open"]` — protrudes 16px past parent div.flex.items-center (no overflow clip)
- 🟡 `deep-chat-long` [mobile] `[data-testid="inline-file-preview-open"]` — protrudes 14px past parent div.flex.items-center (no overflow clip)
- 🟡 `deep-chat-long` [mobile] `[data-testid="inline-file-preview-open"]` — protrudes 16px past parent div.flex.items-center (no overflow clip)
- 🟡 `deep-chat-long` [mobile] `span.text-[var(--sdm-c,inherit)].dark:text-[var(--shiki-dark,var(--sdm-c,inherit))]` — protrudes 7px past parent span.block.before:content-[counter(line)] (no overflow clip)
- 🟡 `deep-chat-long` [mobile] `span.text-[var(--sdm-c,inherit)].dark:text-[var(--shiki-dark,var(--sdm-c,inherit))]` — protrudes 15px past parent span.block.before:content-[counter(line)] (no overflow clip)
- 🟡 `deep-chat-long` [mobile] `span.text-[var(--sdm-c,inherit)].dark:text-[var(--shiki-dark,var(--sdm-c,inherit))]` — protrudes 7px past parent span.block.before:content-[counter(line)] (no overflow clip)
- 🟡 `deep-chat-long` [mobile] `span.text-[var(--sdm-c,inherit)].dark:text-[var(--shiki-dark,var(--sdm-c,inherit))]` — protrudes 15px past parent span.block.before:content-[counter(line)] (no overflow clip)
- 🟡 `deep-chat-long` [mobile] `span.text-[var(--sdm-c,inherit)].dark:text-[var(--shiki-dark,var(--sdm-c,inherit))]` — protrudes 7px past parent span.block.before:content-[counter(line)] (no overflow clip)
- … +151 more (see JSONL)

### A5 (78)

- 🟡 `chat` [desktop] `div.px-3.pt-2.5` — asymmetric vertical padding (top 10px vs bottom 4px) around input content — reads vertically off-center
- 🟡 `chat` [desktop] `div.px-3.pt-2.5` — asymmetric vertical padding (top 10px vs bottom 4px) around input content — reads vertically off-center
- 🟡 `chat` [desktop] `div.px-3.pt-2.5` — asymmetric vertical padding (top 10px vs bottom 4px) around input content — reads vertically off-center
- 🟡 `chat` [mobile] `div.px-3.pt-2.5` — asymmetric vertical padding (top 10px vs bottom 4px) around input content — reads vertically off-center
- 🟡 `chat` [mobile] `div.px-3.pt-2.5` — asymmetric vertical padding (top 10px vs bottom 4px) around input content — reads vertically off-center
- 🟡 `chat` [mobile] `div.px-3.pt-2.5` — asymmetric vertical padding (top 10px vs bottom 4px) around input content — reads vertically off-center
- 🟡 `chat` [tablet] `div.px-3.pt-2.5` — asymmetric vertical padding (top 10px vs bottom 4px) around input content — reads vertically off-center
- 🟡 `chat` [tablet] `div.px-3.pt-2.5` — asymmetric vertical padding (top 10px vs bottom 4px) around input content — reads vertically off-center
- 🟡 `chat` [tablet] `div.px-3.pt-2.5` — asymmetric vertical padding (top 10px vs bottom 4px) around input content — reads vertically off-center
- 🟡 `deep-chat-attachments` [desktop] `div.px-3.pt-2.5` — asymmetric vertical padding (top 10px vs bottom 4px) around input content — reads vertically off-center
- 🟡 `deep-chat-attachments` [mobile] `div.px-3.pt-2.5` — asymmetric vertical padding (top 10px vs bottom 4px) around input content — reads vertically off-center
- 🟡 `deep-chat-attachments` [tablet] `div.px-3.pt-2.5` — asymmetric vertical padding (top 10px vs bottom 4px) around input content — reads vertically off-center
- 🟡 `deep-chat-branched` [desktop] `div.px-3.pt-2.5` — asymmetric vertical padding (top 10px vs bottom 4px) around input content — reads vertically off-center
- 🟡 `deep-chat-branched` [mobile] `div.px-3.pt-2.5` — asymmetric vertical padding (top 10px vs bottom 4px) around input content — reads vertically off-center
- 🟡 `deep-chat-branched` [tablet] `div.px-3.pt-2.5` — asymmetric vertical padding (top 10px vs bottom 4px) around input content — reads vertically off-center
- 🟡 `deep-chat-elicitation` [desktop] `div.px-3.pt-2.5` — asymmetric vertical padding (top 10px vs bottom 4px) around input content — reads vertically off-center
- 🟡 `deep-chat-elicitation` [mobile] `div.px-3.pt-2.5` — asymmetric vertical padding (top 10px vs bottom 4px) around input content — reads vertically off-center
- 🟡 `deep-chat-elicitation` [tablet] `div.px-3.pt-2.5` — asymmetric vertical padding (top 10px vs bottom 4px) around input content — reads vertically off-center
- 🟡 `deep-chat-long` [desktop] `div.px-3.pt-2.5` — asymmetric vertical padding (top 10px vs bottom 4px) around input content — reads vertically off-center
- 🟡 `deep-chat-long` [desktop] `div.px-3.pt-2.5` — asymmetric vertical padding (top 10px vs bottom 4px) around input content — reads vertically off-center
- 🟡 `deep-chat-long` [desktop] `div.px-3.pt-2.5` — asymmetric vertical padding (top 10px vs bottom 4px) around input content — reads vertically off-center
- 🟡 `deep-chat-long` [mobile] `[data-testid="app-header-bar"]` — asymmetric vertical padding (top 0px vs bottom 5px) around input content — reads vertically off-center
- 🟡 `deep-chat-long` [mobile] `div.px-3.pt-2.5` — asymmetric vertical padding (top 10px vs bottom 4px) around input content — reads vertically off-center
- 🟡 `deep-chat-long` [mobile] `div.px-3.pt-2.5` — asymmetric vertical padding (top 10px vs bottom 4px) around input content — reads vertically off-center
- 🟡 `deep-chat-long` [mobile] `div.px-3.pt-2.5` — asymmetric vertical padding (top 10px vs bottom 4px) around input content — reads vertically off-center
- 🟡 `deep-chat-long` [tablet] `div.px-3.pt-2.5` — asymmetric vertical padding (top 10px vs bottom 4px) around input content — reads vertically off-center
- 🟡 `deep-chat-long` [tablet] `div.px-3.pt-2.5` — asymmetric vertical padding (top 10px vs bottom 4px) around input content — reads vertically off-center
- 🟡 `deep-chat-long` [tablet] `div.px-3.pt-2.5` — asymmetric vertical padding (top 10px vs bottom 4px) around input content — reads vertically off-center
- 🟡 `deep-chat-mcp-toolcall-completed` [desktop] `div.px-3.pt-2.5` — asymmetric vertical padding (top 10px vs bottom 4px) around input content — reads vertically off-center
- 🟡 `deep-chat-mcp-toolcall-completed` [mobile] `div.px-3.pt-2.5` — asymmetric vertical padding (top 10px vs bottom 4px) around input content — reads vertically off-center
- … +48 more (see JSONL)

### A8 (6)

- 🟡 `seeded-defect-repro` [desktop] `[data-testid="repro-a8-tablist"]` — strip child button.px-2.text-sm center-y off container center by 8px (vertical mis-centering)
- 🟡 `seeded-defect-repro` [desktop] `[data-testid="repro-i5-tablist"]` — strip child button.px-2.py-1 center-y off container center by 29px (vertical mis-centering)
- 🟡 `seeded-defect-repro` [mobile] `[data-testid="repro-a8-tablist"]` — strip child button.px-2.text-sm center-y off container center by 8px (vertical mis-centering)
- 🟡 `seeded-defect-repro` [mobile] `[data-testid="repro-i5-tablist"]` — strip child button.px-2.py-1 center-y off container center by 29px (vertical mis-centering)
- 🟡 `seeded-defect-repro` [tablet] `[data-testid="repro-a8-tablist"]` — strip child button.px-2.text-sm center-y off container center by 8px (vertical mis-centering)
- 🟡 `seeded-defect-repro` [tablet] `[data-testid="repro-i5-tablist"]` — strip child button.px-2.py-1 center-y off container center by 29px (vertical mis-centering)

### A9 (7)

- 🟡 `overlay-edit-user-group-drawer` [mobile] `li` — peer metric mismatch (element-height): 52px vs group mode 40px among 37 same-kind siblings
- 🟡 `seeded-defect-repro` [desktop] `[data-testid="repro-a9-chip-3"]` — peer metric mismatch (icon-height): 22px vs group mode 14px among 3 same-kind siblings
- 🟡 `seeded-defect-repro` [desktop] `[data-testid="repro-a9-chip-3"]` — peer metric mismatch (icon-width): 22px vs group mode 14px among 3 same-kind siblings
- 🟡 `seeded-defect-repro` [mobile] `[data-testid="repro-a9-chip-3"]` — peer metric mismatch (icon-height): 22px vs group mode 14px among 3 same-kind siblings
- 🟡 `seeded-defect-repro` [mobile] `[data-testid="repro-a9-chip-3"]` — peer metric mismatch (icon-width): 14px vs group mode 10px among 3 same-kind siblings
- 🟡 `seeded-defect-repro` [tablet] `[data-testid="repro-a9-chip-3"]` — peer metric mismatch (icon-height): 22px vs group mode 14px among 3 same-kind siblings
- 🟡 `seeded-defect-repro` [tablet] `[data-testid="repro-a9-chip-3"]` — peer metric mismatch (icon-width): 22px vs group mode 14px among 3 same-kind siblings

### B1 (20)

- 🟡 `deep-project-detail` [mobile] `div[data-slot=card-header].group/card-header.@container/card-header` — premature stack: 2 children on 2 rows but Σwidths+gaps ≈ 192px ≤ container 274px (82px slack — fits on one row)
- 🟡 `deep-project-detail` [mobile] `div[data-slot=card-header].group/card-header.@container/card-header` — premature stack: 2 children on 2 rows but Σwidths+gaps ≈ 186px ≤ container 274px (88px slack — fits on one row)
- 🟡 `deep-project-detail-empty` [mobile] `div[data-slot=card-header].group/card-header.@container/card-header` — premature stack: 2 children on 2 rows but Σwidths+gaps ≈ 192px ≤ container 274px (82px slack — fits on one row)
- 🟡 `deep-project-detail-empty` [mobile] `div[data-slot=card-header].group/card-header.@container/card-header` — premature stack: 2 children on 2 rows but Σwidths+gaps ≈ 186px ≤ container 274px (88px slack — fits on one row)
- 🟡 `seeded-llm-models-loading` [mobile] `div[data-slot=card-header].group/card-header.@container/card-header` — premature stack: 2 children on 2 rows but Σwidths+gaps ≈ 96px ≤ container 306px (210px slack — fits on one row)
- 🟡 `seeded-s4-project-mcp-empty` [mobile] `div[data-slot=card-header].group/card-header.@container/card-header` — premature stack: 2 children on 2 rows but Σwidths+gaps ≈ 186px ≤ container 306px (120px slack — fits on one row)
- 🟡 `seeded-s4-project-mcp-loading` [mobile] `div[data-slot=card-header].group/card-header.@container/card-header` — premature stack: 2 children on 2 rows but Σwidths+gaps ≈ 186px ≤ container 306px (120px slack — fits on one row)
- 🟡 `settings-llm-providers` [mobile] `div[data-slot=card-header].group/card-header.@container/card-header` — premature stack: 2 children on 2 rows but Σwidths+gaps ≈ 96px ≤ container 282px (186px slack — fits on one row)
- 🟡 `settings-llm-repositories` [mobile] `div[data-slot=card-header].group/card-header.@container/card-header` — premature stack: 2 children on 2 rows but Σwidths+gaps ≈ 191px ≤ container 282px (91px slack — fits on one row)
- 🟡 `settings-llm-repositories` [mobile] `div[data-slot=card-header].group/card-header.@container/card-header` — premature stack: 2 children on 2 rows but Σwidths+gaps ≈ 191px ≤ container 282px (91px slack — fits on one row)
- 🟡 `settings-llm-repositories` [mobile] `div[data-slot=card-header].group/card-header.@container/card-header` — premature stack: 2 children on 2 rows but Σwidths+gaps ≈ 191px ≤ container 282px (91px slack — fits on one row)
- 🟡 `settings-llm-runtime` [mobile] `div[data-slot=card-header].group/card-header.@container/card-header` — premature stack: 2 children on 2 rows but Σwidths+gaps ≈ 245px ≤ container 282px (37px slack — fits on one row)
- 🟡 `settings-llm-runtime` [mobile] `div[data-slot=card-header].group/card-header.@container/card-header` — premature stack: 2 children on 2 rows but Σwidths+gaps ≈ 245px ≤ container 282px (37px slack — fits on one row)
- 🟡 `settings-llm-runtime` [mobile] `div[data-slot=card-header].group/card-header.@container/card-header` — premature stack: 2 children on 2 rows but Σwidths+gaps ≈ 245px ≤ container 282px (37px slack — fits on one row)
- 🟡 `settings-memory` [mobile] `div[data-slot=card-header].group/card-header.@container/card-header` — premature stack: 2 children on 2 rows but Σwidths+gaps ≈ 148px ≤ container 282px (134px slack — fits on one row)
- 🟡 `settings-memory` [mobile] `div[data-slot=card-header].group/card-header.@container/card-header` — premature stack: 2 children on 2 rows but Σwidths+gaps ≈ 148px ≤ container 282px (134px slack — fits on one row)
- 🟡 `settings-memory` [mobile] `div[data-slot=card-header].group/card-header.@container/card-header` — premature stack: 2 children on 2 rows but Σwidths+gaps ≈ 148px ≤ container 282px (134px slack — fits on one row)
- 🟡 `settings-users` [mobile] `div[data-slot=card-header].group/card-header.@container/card-header` — premature stack: 2 children on 2 rows but Σwidths+gaps ≈ 85px ≤ container 282px (197px slack — fits on one row)
- 🟡 `settings-users` [mobile] `div[data-slot=card-header].group/card-header.@container/card-header` — premature stack: 2 children on 2 rows but Σwidths+gaps ≈ 85px ≤ container 282px (197px slack — fits on one row)
- 🟡 `settings-users` [mobile] `div[data-slot=card-header].group/card-header.@container/card-header` — premature stack: 2 children on 2 rows but Σwidths+gaps ≈ 85px ≤ container 282px (197px slack — fits on one row)

### C1 (6)

- 🟡 `seeded-defect-repro` [desktop] `[data-testid="repro-c1-badge"]` — status badge "verified" ordered BEFORE its label "vaswani2017attention" — a badge should FOLLOW the thing it qualifies
- 🟡 `seeded-defect-repro` [mobile] `[data-testid="repro-c1-badge"]` — status badge "verified" ordered BEFORE its label "vaswani2017attention" — a badge should FOLLOW the thing it qualifies
- 🟡 `seeded-defect-repro` [tablet] `[data-testid="repro-c1-badge"]` — status badge "verified" ordered BEFORE its label "vaswani2017attention" — a badge should FOLLOW the thing it qualifies
- 🟡 `seeded-s1-run-progress-error` [desktop] `[data-testid="wf-progress-status-tag"]` — status badge "failed" ordered BEFORE its label "1,840 tokens" — a badge should FOLLOW the thing it qualifies
- 🟡 `seeded-s1-run-progress-error` [mobile] `[data-testid="wf-progress-status-tag"]` — status badge "failed" ordered BEFORE its label "1,840 tokens" — a badge should FOLLOW the thing it qualifies
- 🟡 `seeded-s1-run-progress-error` [tablet] `[data-testid="wf-progress-status-tag"]` — status badge "failed" ordered BEFORE its label "1,840 tokens" — a badge should FOLLOW the thing it qualifies

### C10 (15)

- 🟡 `chats` [desktop] `svg` — icon height 64px is 2.00× the adjacent text line-height 32px (oversized)
- 🟡 `chats` [mobile] `svg` — icon height 64px is 2.00× the adjacent text line-height 32px (oversized)
- 🟡 `chats` [tablet] `svg` — icon height 64px is 2.00× the adjacent text line-height 32px (oversized)
- 🟡 `deep-chat-right-panel-file` [desktop] `svg` — icon height 32px is 2.00× the adjacent text line-height 16px (oversized)
- 🟡 `deep-chat-right-panel-file` [mobile] `svg` — icon height 32px is 2.00× the adjacent text line-height 16px (oversized)
- 🟡 `deep-chat-right-panel-file` [tablet] `svg` — icon height 32px is 2.00× the adjacent text line-height 16px (oversized)
- 🟡 `deep-project-detail-error` [desktop] `svg` — icon height 48px is 1.71× the adjacent text line-height 28px (oversized)
- 🟡 `deep-project-detail-error` [mobile] `svg` — icon height 48px is 1.71× the adjacent text line-height 28px (oversized)
- 🟡 `deep-project-detail-error` [tablet] `svg` — icon height 48px is 1.71× the adjacent text line-height 28px (oversized)
- 🟡 `projects` [desktop] `svg` — icon height 64px is 2.00× the adjacent text line-height 32px (oversized)
- 🟡 `projects` [mobile] `svg` — icon height 64px is 2.00× the adjacent text line-height 32px (oversized)
- 🟡 `projects` [tablet] `svg` — icon height 64px is 2.00× the adjacent text line-height 32px (oversized)
- 🟡 `seeded-defect-repro` [desktop] `[data-testid="repro-c10-icon"]` — icon height 48px is 2.40× the adjacent text line-height 20px (oversized)
- 🟡 `seeded-defect-repro` [mobile] `[data-testid="repro-c10-icon"]` — icon height 48px is 2.40× the adjacent text line-height 20px (oversized)
- 🟡 `seeded-defect-repro` [tablet] `[data-testid="repro-c10-icon"]` — icon height 48px is 2.40× the adjacent text line-height 20px (oversized)

### C12 (3)

- 🟡 `seeded-defect-repro` [desktop] `[data-testid="repro-c12-avatar"]` — bare placeholder circle: rounded-full 40×40px with no img/svg/initials content
- 🟡 `seeded-defect-repro` [mobile] `[data-testid="repro-c12-avatar"]` — bare placeholder circle: rounded-full 40×40px with no img/svg/initials content
- 🟡 `seeded-defect-repro` [tablet] `[data-testid="repro-c12-avatar"]` — bare placeholder circle: rounded-full 40×40px with no img/svg/initials content

### C7 (3)

- 🟡 `seeded-defect-repro` [desktop] `[data-role="repro-usr"] vs [data-role="repro-asst"]` — two DIFFERENT roles ("repro-usr" vs "repro-asst") render with an IDENTICAL visual signature (bg=32,32,32|align=center|border=0|avatar=0) — reader can't tell them apart
- 🟡 `seeded-defect-repro` [mobile] `[data-role="repro-usr"] vs [data-role="repro-asst"]` — two DIFFERENT roles ("repro-usr" vs "repro-asst") render with an IDENTICAL visual signature (bg=32,32,32|align=center|border=0|avatar=0) — reader can't tell them apart
- 🟡 `seeded-defect-repro` [tablet] `[data-role="repro-usr"] vs [data-role="repro-asst"]` — two DIFFERENT roles ("repro-usr" vs "repro-asst") render with an IDENTICAL visual signature (bg=32,32,32|align=center|border=0|avatar=0) — reader can't tell them apart

### C9 (3)

- 🟡 `seeded-defect-repro` [desktop] `[data-testid="repro-c9-row"]` — icon and its label "Tool Approval Requir" render on different lines (disjoint y) though 205px fits container 260px
- 🟡 `seeded-defect-repro` [mobile] `[data-testid="repro-c9-row"]` — icon and its label "Tool Approval Requir" render on different lines (disjoint y) though 205px fits container 260px
- 🟡 `seeded-defect-repro` [tablet] `[data-testid="repro-c9-row"]` — icon and its label "Tool Approval Requir" render on different lines (disjoint y) though 205px fits container 260px

### D1 (114)

- 🟡 `chat` [desktop] `span[data-slot=select-value].flex.flex-1` — text truncated (hidden 6px) but parent has 40px free — could show "Select Model"
- 🟡 `chat` [desktop] `span[data-slot=select-value].flex.flex-1` — text truncated (hidden 6px) but parent has 40px free — could show "Select Model"
- 🟡 `chat` [mobile] `span[data-slot=select-value].flex.flex-1` — text truncated (hidden 6px) but parent has 40px free — could show "Select Model"
- 🟡 `chat` [mobile] `span[data-slot=select-value].flex.flex-1` — text truncated (hidden 6px) but parent has 40px free — could show "Select Model"
- 🟡 `chat` [tablet] `span[data-slot=select-value].flex.flex-1` — text truncated (hidden 6px) but parent has 40px free — could show "Select Model"
- 🟡 `chat` [tablet] `span[data-slot=select-value].flex.flex-1` — text truncated (hidden 6px) but parent has 40px free — could show "Select Model"
- 🟡 `deep-chat-attachments` [desktop] `span[data-slot=select-value].flex.flex-1` — text truncated (hidden 6px) but parent has 40px free — could show "Select Model"
- 🟡 `deep-chat-attachments` [mobile] `[data-testid="conversation-title"]` — text truncated (hidden 61px) but parent has 72px free — could show "Message with attachments"
- 🟡 `deep-chat-attachments` [mobile] `span[data-slot=select-value].flex.flex-1` — text truncated (hidden 6px) but parent has 40px free — could show "Select Model"
- 🟡 `deep-chat-attachments` [tablet] `span[data-slot=select-value].flex.flex-1` — text truncated (hidden 6px) but parent has 40px free — could show "Select Model"
- 🟡 `deep-chat-branched` [desktop] `span[data-slot=select-value].flex.flex-1` — text truncated (hidden 6px) but parent has 40px free — could show "Select Model"
- 🟡 `deep-chat-branched` [mobile] `span[data-slot=select-value].flex.flex-1` — text truncated (hidden 6px) but parent has 40px free — could show "Select Model"
- 🟡 `deep-chat-branched` [tablet] `span[data-slot=select-value].flex.flex-1` — text truncated (hidden 6px) but parent has 40px free — could show "Select Model"
- 🟡 `deep-chat-elicitation` [desktop] `span[data-slot=select-value].flex.flex-1` — text truncated (hidden 6px) but parent has 40px free — could show "Select Model"
- 🟡 `deep-chat-elicitation` [mobile] `[data-testid="conversation-title"]` — text truncated (hidden 65px) but parent has 72px free — could show "Elicitation — awaiting i"
- 🟡 `deep-chat-elicitation` [mobile] `span[data-slot=select-value].flex.flex-1` — text truncated (hidden 6px) but parent has 40px free — could show "Select Model"
- 🟡 `deep-chat-elicitation` [tablet] `span[data-slot=select-value].flex.flex-1` — text truncated (hidden 6px) but parent has 40px free — could show "Select Model"
- 🟡 `deep-chat-long` [desktop] `span[data-slot=select-value].flex.flex-1` — text truncated (hidden 6px) but parent has 40px free — could show "Select Model"
- 🟡 `deep-chat-long` [desktop] `span[data-slot=select-value].flex.flex-1` — text truncated (hidden 6px) but parent has 40px free — could show "Select Model"
- 🟡 `deep-chat-long` [desktop] `span[data-slot=select-value].flex.flex-1` — text truncated (hidden 6px) but parent has 40px free — could show "Select Model"
- 🟡 `deep-chat-long` [mobile] `span.font-medium.truncate` — text truncated (hidden 49px) but parent has 237px free — could show "report.pdf"
- 🟡 `deep-chat-long` [mobile] `span.font-medium.truncate` — text truncated (hidden 49px) but parent has 237px free — could show "report.pdf"
- 🟡 `deep-chat-long` [mobile] `span.font-medium.truncate` — text truncated (hidden 49px) but parent has 237px free — could show "report.pdf"
- 🟡 `deep-chat-long` [mobile] `span.text-sm.font-semibold` — text truncated (hidden 60px) but parent has 168px free — could show "get_weather"
- 🟡 `deep-chat-long` [mobile] `span.text-sm.font-semibold` — text truncated (hidden 60px) but parent has 168px free — could show "get_weather"
- 🟡 `deep-chat-long` [mobile] `span.text-sm.font-semibold` — text truncated (hidden 60px) but parent has 168px free — could show "get_weather"
- 🟡 `deep-chat-long` [mobile] `span[data-slot=select-value].flex.flex-1` — text truncated (hidden 6px) but parent has 40px free — could show "Select Model"
- 🟡 `deep-chat-long` [mobile] `span[data-slot=select-value].flex.flex-1` — text truncated (hidden 6px) but parent has 40px free — could show "Select Model"
- 🟡 `deep-chat-long` [mobile] `span[data-slot=select-value].flex.flex-1` — text truncated (hidden 6px) but parent has 40px free — could show "Select Model"
- 🟡 `deep-chat-long` [tablet] `span[data-slot=select-value].flex.flex-1` — text truncated (hidden 6px) but parent has 40px free — could show "Select Model"
- … +84 more (see JSONL)

### G5 (286)

- 🟡 `auth` [mobile] `button.pointer-events-auto.text-muted-foreground` — tap target 16×16px < 44px (mobile)
- 🟡 `auth` [mobile] `button.pointer-events-auto.text-muted-foreground` — tap target 16×16px < 44px (mobile)
- 🟡 `auth` [mobile] `button.pointer-events-auto.text-muted-foreground` — tap target 16×16px < 44px (mobile)
- 🟡 `auth-link-account` [mobile] `a` — tap target 47×16px < 44px (mobile)
- 🟡 `auth-link-account` [mobile] `button.pointer-events-auto.text-muted-foreground` — tap target 16×16px < 44px (mobile)
- 🟡 `chat` [mobile] `[data-testid="mcp-chip-e0000000-0000-0000-0000-0000000000e1-close"]` — tap target 12×12px < 44px (mobile)
- 🟡 `chats` [mobile] `[data-testid="chat-conversation-select-11111111-1111-1111-1111-111111111111"]` — tap target 16×16px < 44px (mobile)
- 🟡 `deep-chat-attachments` [mobile] `[data-testid="mcp-chip-e0000000-0000-0000-0000-0000000000e1-close"]` — tap target 12×12px < 44px (mobile)
- 🟡 `deep-chat-attachments` [mobile] `[data-testid="memory-status-pill"]` — tap target 117×22px < 44px (mobile)
- 🟡 `deep-chat-attachments` [mobile] `[data-testid="summ-mode-tag"]` — tap target 126×22px < 44px (mobile)
- 🟡 `deep-chat-branched` [mobile] `[data-testid="mcp-chip-e0000000-0000-0000-0000-0000000000e1-close"]` — tap target 12×12px < 44px (mobile)
- 🟡 `deep-chat-branched` [mobile] `[data-testid="memory-status-pill"]` — tap target 117×22px < 44px (mobile)
- 🟡 `deep-chat-branched` [mobile] `[data-testid="summ-mode-tag"]` — tap target 126×22px < 44px (mobile)
- 🟡 `deep-chat-elicitation` [mobile] `[data-testid="elicitation-field-include_headers"]` — tap target 32×18px < 44px (mobile)
- 🟡 `deep-chat-elicitation` [mobile] `[data-testid="mcp-chip-e0000000-0000-0000-0000-0000000000e1-close"]` — tap target 12×12px < 44px (mobile)
- 🟡 `deep-chat-elicitation` [mobile] `[data-testid="memory-status-pill"]` — tap target 117×22px < 44px (mobile)
- 🟡 `deep-chat-elicitation` [mobile] `[data-testid="summ-mode-tag"]` — tap target 126×22px < 44px (mobile)
- 🟡 `deep-chat-long` [mobile] `[data-testid="mcp-chip-e0000000-0000-0000-0000-0000000000e1-close"]` — tap target 12×12px < 44px (mobile)
- 🟡 `deep-chat-long` [mobile] `[data-testid="memory-status-pill"]` — tap target 117×22px < 44px (mobile)
- 🟡 `deep-chat-long` [mobile] `[data-testid="memory-status-pill"]` — tap target 117×22px < 44px (mobile)
- 🟡 `deep-chat-long` [mobile] `[data-testid="memory-status-pill"]` — tap target 117×22px < 44px (mobile)
- 🟡 `deep-chat-long` [mobile] `[data-testid="summ-mode-tag"]` — tap target 126×22px < 44px (mobile)
- 🟡 `deep-chat-long` [mobile] `[data-testid="summ-mode-tag"]` — tap target 126×22px < 44px (mobile)
- 🟡 `deep-chat-long` [mobile] `[data-testid="summ-mode-tag"]` — tap target 126×22px < 44px (mobile)
- 🟡 `deep-chat-long` [mobile] `a.wrap-anywhere.font-medium` — tap target 28×19px < 44px (mobile)
- 🟡 `deep-chat-long` [mobile] `a.wrap-anywhere.font-medium` — tap target 28×19px < 44px (mobile)
- 🟡 `deep-chat-long` [mobile] `a.wrap-anywhere.font-medium` — tap target 28×19px < 44px (mobile)
- 🟡 `deep-chat-long` [mobile] `a#user-content-user-content-fnref-1-2.wrap-anywhere.font-medium` — tap target 9×16px < 44px (mobile)
- 🟡 `deep-chat-long` [mobile] `a#user-content-user-content-fnref-1-2.wrap-anywhere.font-medium` — tap target 9×16px < 44px (mobile)
- 🟡 `deep-chat-long` [mobile] `a#user-content-user-content-fnref-1-2.wrap-anywhere.font-medium` — tap target 9×16px < 44px (mobile)
- … +256 more (see JSONL)

### G7 (128)

- 🟡 `deep-chat-long` [desktop] `a.wrap-anywhere.font-medium` — focus ring (2px) clipped by overflow-x ancestor div.w-full.overflow-x-auto — cut 2px
- 🟡 `deep-chat-long` [desktop] `a.wrap-anywhere.font-medium` — focus ring (2px) clipped by overflow-x ancestor div.w-full.overflow-x-auto — cut 2px
- 🟡 `deep-chat-long` [desktop] `a.wrap-anywhere.font-medium` — focus ring (2px) clipped by overflow-x ancestor div.w-full.overflow-x-auto — cut 2px
- 🟡 `deep-chat-long` [desktop] `a.wrap-anywhere.font-medium` — focus ring (2px) clipped by overflow-x ancestor div.w-full.overflow-x-auto — cut 2px
- 🟡 `deep-chat-long` [desktop] `a.wrap-anywhere.font-medium` — focus ring (2px) clipped by overflow-x ancestor div.w-full.overflow-x-auto — cut 2px
- 🟡 `deep-chat-long` [desktop] `a.wrap-anywhere.font-medium` — focus ring (2px) clipped by overflow-x ancestor div.w-full.overflow-x-auto — cut 2px
- 🟡 `deep-chat-long` [desktop] `a.wrap-anywhere.font-medium` — focus ring (2px) clipped by overflow-x ancestor div.w-full.overflow-x-auto — cut 2px
- 🟡 `deep-chat-long` [desktop] `a.wrap-anywhere.font-medium` — focus ring (2px) clipped by overflow-x ancestor div.w-full.overflow-x-auto — cut 2px
- 🟡 `deep-chat-long` [desktop] `a.wrap-anywhere.font-medium` — focus ring (2px) clipped by overflow-x ancestor div.w-full.overflow-x-auto — cut 2px
- 🟡 `deep-chat-long` [mobile] `a.wrap-anywhere.font-medium` — focus ring (2px) clipped by overflow-x ancestor div.w-full.overflow-x-auto — cut 2px
- 🟡 `deep-chat-long` [mobile] `a.wrap-anywhere.font-medium` — focus ring (2px) clipped by overflow-x ancestor div.w-full.overflow-x-auto — cut 2px
- 🟡 `deep-chat-long` [mobile] `a.wrap-anywhere.font-medium` — focus ring (2px) clipped by overflow-x ancestor div.w-full.overflow-x-auto — cut 2px
- 🟡 `deep-chat-long` [mobile] `a.wrap-anywhere.font-medium` — focus ring (2px) clipped by overflow-x ancestor div.w-full.overflow-x-auto — cut 2px
- 🟡 `deep-chat-long` [mobile] `a.wrap-anywhere.font-medium` — focus ring (2px) clipped by overflow-x ancestor div.w-full.overflow-x-auto — cut 2px
- 🟡 `deep-chat-long` [mobile] `a.wrap-anywhere.font-medium` — focus ring (2px) clipped by overflow-x ancestor div.w-full.overflow-x-auto — cut 2px
- 🟡 `deep-chat-long` [mobile] `a.wrap-anywhere.font-medium` — focus ring (2px) clipped by overflow-x ancestor div.w-full.overflow-x-auto — cut 2px
- 🟡 `deep-chat-long` [mobile] `a.wrap-anywhere.font-medium` — focus ring (2px) clipped by overflow-x ancestor div.w-full.overflow-x-auto — cut 2px
- 🟡 `deep-chat-long` [mobile] `a.wrap-anywhere.font-medium` — focus ring (2px) clipped by overflow-x ancestor div.w-full.overflow-x-auto — cut 2px
- 🟡 `deep-chat-long` [mobile] `a#user-content-user-content-fnref-1.wrap-anywhere.font-medium` — focus ring (2px) clipped by overflow-x ancestor div — cut 372px
- 🟡 `deep-chat-long` [mobile] `a#user-content-user-content-fnref-1.wrap-anywhere.font-medium` — focus ring (2px) clipped by overflow-x ancestor div — cut 372px
- 🟡 `deep-chat-long` [mobile] `a#user-content-user-content-fnref-1.wrap-anywhere.font-medium` — focus ring (2px) clipped by overflow-x ancestor div — cut 372px
- 🟡 `deep-chat-long` [tablet] `a.wrap-anywhere.font-medium` — focus ring (2px) clipped by overflow-x ancestor div.w-full.overflow-x-auto — cut 2px
- 🟡 `deep-chat-long` [tablet] `a.wrap-anywhere.font-medium` — focus ring (2px) clipped by overflow-x ancestor div.w-full.overflow-x-auto — cut 2px
- 🟡 `deep-chat-long` [tablet] `a.wrap-anywhere.font-medium` — focus ring (2px) clipped by overflow-x ancestor div.w-full.overflow-x-auto — cut 2px
- 🟡 `deep-chat-long` [tablet] `a.wrap-anywhere.font-medium` — focus ring (2px) clipped by overflow-x ancestor div.w-full.overflow-x-auto — cut 2px
- 🟡 `deep-chat-long` [tablet] `a.wrap-anywhere.font-medium` — focus ring (2px) clipped by overflow-x ancestor div.w-full.overflow-x-auto — cut 2px
- 🟡 `deep-chat-long` [tablet] `a.wrap-anywhere.font-medium` — focus ring (2px) clipped by overflow-x ancestor div.w-full.overflow-x-auto — cut 2px
- 🟡 `deep-chat-no-models` [desktop] `a.wrap-anywhere.font-medium` — focus ring (2px) clipped by overflow-x ancestor div.w-full.overflow-x-auto — cut 2px
- 🟡 `deep-chat-no-models` [desktop] `a.wrap-anywhere.font-medium` — focus ring (2px) clipped by overflow-x ancestor div.w-full.overflow-x-auto — cut 2px
- 🟡 `deep-chat-no-models` [desktop] `a.wrap-anywhere.font-medium` — focus ring (2px) clipped by overflow-x ancestor div.w-full.overflow-x-auto — cut 2px
- … +98 more (see JSONL)

### G9 (9)

- 🟡 `deep-project-detail` [desktop] `div[data-slot=input-group-addon].flex.h-auto` — 1 hover-reveal control(s) use display:none (reserve NO layout space) beside 1 persistent sibling(s) — the persistent element shifts when hover controls appear; reserve space via visibility/opacity
- 🟡 `deep-project-detail` [mobile] `div[data-slot=input-group-addon].flex.h-auto` — 1 hover-reveal control(s) use display:none (reserve NO layout space) beside 1 persistent sibling(s) — the persistent element shifts when hover controls appear; reserve space via visibility/opacity
- 🟡 `deep-project-detail` [tablet] `div[data-slot=input-group-addon].flex.h-auto` — 1 hover-reveal control(s) use display:none (reserve NO layout space) beside 1 persistent sibling(s) — the persistent element shifts when hover controls appear; reserve space via visibility/opacity
- 🟡 `deep-project-detail-empty` [desktop] `div[data-slot=input-group-addon].flex.h-auto` — 1 hover-reveal control(s) use display:none (reserve NO layout space) beside 1 persistent sibling(s) — the persistent element shifts when hover controls appear; reserve space via visibility/opacity
- 🟡 `deep-project-detail-empty` [mobile] `div[data-slot=input-group-addon].flex.h-auto` — 1 hover-reveal control(s) use display:none (reserve NO layout space) beside 1 persistent sibling(s) — the persistent element shifts when hover controls appear; reserve space via visibility/opacity
- 🟡 `deep-project-detail-empty` [tablet] `div[data-slot=input-group-addon].flex.h-auto` — 1 hover-reveal control(s) use display:none (reserve NO layout space) beside 1 persistent sibling(s) — the persistent element shifts when hover controls appear; reserve space via visibility/opacity
- 🟡 `seeded-defect-repro` [desktop] `[data-testid="repro-g9-row"]` — 2 hover-reveal control(s) use display:none (reserve NO layout space) beside 1 persistent sibling(s) — the persistent element shifts when hover controls appear; reserve space via visibility/opacity
- 🟡 `seeded-defect-repro` [mobile] `[data-testid="repro-g9-row"]` — 2 hover-reveal control(s) use display:none (reserve NO layout space) beside 1 persistent sibling(s) — the persistent element shifts when hover controls appear; reserve space via visibility/opacity
- 🟡 `seeded-defect-repro` [tablet] `[data-testid="repro-g9-row"]` — 2 hover-reveal control(s) use display:none (reserve NO layout space) beside 1 persistent sibling(s) — the persistent element shifts when hover controls appear; reserve space via visibility/opacity

### H7 (51)

- 🟡 `deep-chat-no-models` [desktop] `[data-testid="ullm-model-select"]` — picker dropdown renders nothing: 0 options + no empty-state hint ([data-testid="ullm-model-select-popup"]) — the user sees literally nothing to select
- 🟡 `deep-chat-no-models` [mobile] `[data-testid="ullm-model-select"]` — picker dropdown renders nothing: 0 options + no empty-state hint ([data-testid="ullm-model-select-popup"]) — the user sees literally nothing to select
- 🟡 `deep-chat-no-models` [tablet] `[data-testid="ullm-model-select"]` — picker dropdown renders nothing: 0 options + no empty-state hint ([data-testid="ullm-model-select-popup"]) — the user sees literally nothing to select
- 🟡 `deep-project-detail` [desktop] `[data-testid="project-default-assistant-combobox"]` — combobox trigger renders NO text / placeholder / icon — an empty control the user needs must say SOMETHING
- 🟡 `deep-project-detail` [desktop] `[data-testid="project-default-model-combobox"]` — combobox trigger renders NO text / placeholder / icon — an empty control the user needs must say SOMETHING
- 🟡 `deep-project-detail` [mobile] `[data-testid="project-default-assistant-combobox"]` — combobox trigger renders NO text / placeholder / icon — an empty control the user needs must say SOMETHING
- 🟡 `deep-project-detail` [mobile] `[data-testid="project-default-model-combobox"]` — combobox trigger renders NO text / placeholder / icon — an empty control the user needs must say SOMETHING
- 🟡 `deep-project-detail` [tablet] `[data-testid="project-default-assistant-combobox"]` — combobox trigger renders NO text / placeholder / icon — an empty control the user needs must say SOMETHING
- 🟡 `deep-project-detail` [tablet] `[data-testid="project-default-model-combobox"]` — combobox trigger renders NO text / placeholder / icon — an empty control the user needs must say SOMETHING
- 🟡 `deep-project-detail-empty` [desktop] `[data-testid="project-default-assistant-combobox"]` — combobox trigger renders NO text / placeholder / icon — an empty control the user needs must say SOMETHING
- 🟡 `deep-project-detail-empty` [desktop] `[data-testid="project-default-model-combobox"]` — combobox trigger renders NO text / placeholder / icon — an empty control the user needs must say SOMETHING
- 🟡 `deep-project-detail-empty` [mobile] `[data-testid="project-default-assistant-combobox"]` — combobox trigger renders NO text / placeholder / icon — an empty control the user needs must say SOMETHING
- 🟡 `deep-project-detail-empty` [mobile] `[data-testid="project-default-model-combobox"]` — combobox trigger renders NO text / placeholder / icon — an empty control the user needs must say SOMETHING
- 🟡 `deep-project-detail-empty` [tablet] `[data-testid="project-default-assistant-combobox"]` — combobox trigger renders NO text / placeholder / icon — an empty control the user needs must say SOMETHING
- 🟡 `deep-project-detail-empty` [tablet] `[data-testid="project-default-model-combobox"]` — combobox trigger renders NO text / placeholder / icon — an empty control the user needs must say SOMETHING
- 🟡 `overlay-add-to-project-modal` [desktop] `[data-testid="project-add-to-project-combobox"]` — combobox trigger renders NO text / placeholder / icon — an empty control the user needs must say SOMETHING
- 🟡 `overlay-add-to-project-modal` [mobile] `[data-testid="project-add-to-project-combobox"]` — combobox trigger renders NO text / placeholder / icon — an empty control the user needs must say SOMETHING
- 🟡 `overlay-add-to-project-modal` [tablet] `[data-testid="project-add-to-project-combobox"]` — combobox trigger renders NO text / placeholder / icon — an empty control the user needs must say SOMETHING
- 🟡 `seeded-defect-repro` [desktop] `[data-testid="repro-h7-combobox"]` — combobox trigger renders NO text / placeholder / icon — an empty control the user needs must say SOMETHING
- 🟡 `seeded-defect-repro` [desktop] `[data-testid="repro-h7-select"]` — <select> has zero <option>s — renders nothing to pick (empty control must say something)
- 🟡 `seeded-defect-repro` [mobile] `[data-testid="repro-h7-combobox"]` — combobox trigger renders NO text / placeholder / icon — an empty control the user needs must say SOMETHING
- 🟡 `seeded-defect-repro` [mobile] `[data-testid="repro-h7-select"]` — <select> has zero <option>s — renders nothing to pick (empty control must say something)
- 🟡 `seeded-defect-repro` [tablet] `[data-testid="repro-h7-combobox"]` — combobox trigger renders NO text / placeholder / icon — an empty control the user needs must say SOMETHING
- 🟡 `seeded-defect-repro` [tablet] `[data-testid="repro-h7-select"]` — <select> has zero <option>s — renders nothing to pick (empty control must say something)
- 🟡 `seeded-file-rag-error` [desktop] `[data-testid="filerag-embedding-model-select"]` — combobox trigger renders NO text / placeholder / icon — an empty control the user needs must say SOMETHING
- 🟡 `seeded-file-rag-error` [mobile] `[data-testid="filerag-embedding-model-select"]` — combobox trigger renders NO text / placeholder / icon — an empty control the user needs must say SOMETHING
- 🟡 `seeded-file-rag-error` [tablet] `[data-testid="filerag-embedding-model-select"]` — combobox trigger renders NO text / placeholder / icon — an empty control the user needs must say SOMETHING
- 🟡 `settings-file-rag-admin` [desktop] `[data-testid="filerag-embedding-model-select"]` — combobox trigger renders NO text / placeholder / icon — an empty control the user needs must say SOMETHING
- 🟡 `settings-file-rag-admin` [desktop] `[data-testid="filerag-embedding-model-select"]` — combobox trigger renders NO text / placeholder / icon — an empty control the user needs must say SOMETHING
- 🟡 `settings-file-rag-admin` [mobile] `[data-testid="filerag-embedding-model-select"]` — combobox trigger renders NO text / placeholder / icon — an empty control the user needs must say SOMETHING
- … +21 more (see JSONL)

### I1 (159)

- 🟡 `chat` [mobile] `[data-testid="chat-input-add-btn"]` — interactive <button> "" occluded at center by span.[&_svg]:size-4 (hit-test miss)
- 🟡 `deep-chat-long` [mobile] `a.wrap-anywhere.font-medium` — interactive <a> "link" occluded at center by div.flex.justify-between (hit-test miss)
- 🟡 `deep-chat-long` [mobile] `a.wrap-anywhere.font-medium` — interactive <a> "link" occluded at center by div.flex.justify-between (hit-test miss)
- 🟡 `deep-chat-no-models` [mobile] `a.wrap-anywhere.font-medium` — interactive <a> "link" occluded at center by div.flex.justify-between (hit-test miss)
- 🟡 `deep-chat-right-panel-file` [mobile] `[data-testid="chat-branch-next-btn"]` — interactive <button> "" occluded at center by [data-testid="file-pdf-page-error-1"] (hit-test miss)
- 🟡 `deep-chat-right-panel-file` [mobile] `[data-testid="chat-branch-prev-btn"]` — interactive <button> "" occluded at center by [data-testid="file-pdf-page-error-1"] (hit-test miss)
- 🟡 `deep-chat-right-panel-file` [mobile] `[data-testid="chat-input-add-btn"]` — interactive <button> "" occluded at center by [data-testid="file-pdf-page-error-1"] (hit-test miss)
- 🟡 `deep-chat-right-panel-file` [mobile] `[data-testid="chat-input-send-btn"]` — interactive <button> "" occluded at center by [data-testid="file-pdf-page-error-1"] (hit-test miss)
- 🟡 `deep-chat-right-panel-file` [mobile] `[data-testid="chat-message-copy-btn"]` — interactive <button> "" occluded at center by [data-testid="file-pdf-page-error-1"] (hit-test miss)
- 🟡 `deep-chat-right-panel-file` [mobile] `[data-testid="chat-message-textarea"]` — interactive <textarea> "" occluded at center by [data-testid="file-pdf-page-error-1"] (hit-test miss)
- 🟡 `deep-chat-right-panel-file` [mobile] `[data-testid="chat-title-edit-btn"]` — interactive <button> "" occluded at center by div.flex.flex-col (hit-test miss)
- 🟡 `deep-chat-right-panel-file` [mobile] `[data-testid="conversation-back-button"]` — interactive <button> "" occluded at center by div.flex.flex-col (hit-test miss)
- 🟡 `deep-chat-right-panel-file` [mobile] `[data-testid="edit-message-button"]` — interactive <button> "" occluded at center by [data-testid="file-pdf-page-error-1"] (hit-test miss)
- 🟡 `deep-chat-right-panel-file` [mobile] `[data-testid="file-card-remove-btn"]` — interactive <button> "" occluded at center by [data-testid="file-pdf-page-error-1"] (hit-test miss)
- 🟡 `deep-chat-right-panel-file` [mobile] `[data-testid="file-viewer-download-btn"]` — interactive <button> "" occluded at center by div[data-slot=tooltip-content].z-50.inline-flex (hit-test miss)
- 🟡 `deep-chat-right-panel-file` [mobile] `[data-testid="memory-status-pill"]` — interactive <span> "Memory: auto" occluded at center by [data-testid="file-pdf-page-error-1"] (hit-test miss)
- 🟡 `deep-chat-right-panel-file` [mobile] `[data-testid="summ-mode-tag"]` — interactive <span> "Summary: auto" occluded at center by [data-testid="file-pdf-page-error-1"] (hit-test miss)
- 🟡 `deep-chat-right-panel-file` [mobile] `[data-testid="ullm-model-select"]` — interactive <button> "Select Model▼" occluded at center by [data-testid="file-pdf-page-error-1"] (hit-test miss)
- 🟡 `deep-chat-right-panel-file` [mobile] `a.wrap-anywhere.font-medium` — interactive <a> "link" occluded at center by [data-testid="file-pdf-page-error-1"] (hit-test miss)
- 🟡 `deep-chat-right-panel-file` [mobile] `button[data-slot=attachment-trigger].absolute.inset-0` — interactive <button> "" occluded at center by [data-testid="file-pdf-page-error-1"] (hit-test miss)
- 🟡 `deep-chat-right-panel-file` [tablet] `[data-testid="chat-input-add-btn"]` — interactive <button> "" occluded at center by [data-testid="ullm-model-select"] (hit-test miss)
- 🟡 `deep-chat-right-panel-literature` [mobile] `[data-testid="chat-input-add-btn"]` — interactive <button> "" occluded at center by div.flex-1.overflow-hidden (hit-test miss)
- 🟡 `deep-chat-right-panel-literature` [mobile] `[data-testid="chat-input-send-btn"]` — interactive <button> "" occluded at center by div.flex-1.overflow-hidden (hit-test miss)
- 🟡 `deep-chat-right-panel-literature` [mobile] `[data-testid="chat-message-textarea"]` — interactive <textarea> "" occluded at center by div.flex-1.overflow-hidden (hit-test miss)
- 🟡 `deep-chat-right-panel-literature` [mobile] `[data-testid="memory-status-pill"]` — interactive <span> "Memory: auto" occluded at center by div.flex-1.overflow-hidden (hit-test miss)
- 🟡 `deep-chat-right-panel-literature` [mobile] `[data-testid="summ-mode-tag"]` — interactive <span> "Summary: auto" occluded at center by div.flex-1.overflow-hidden (hit-test miss)
- 🟡 `deep-chat-right-panel-literature` [mobile] `[data-testid="ullm-model-select"]` — interactive <button> "Select Model▼" occluded at center by div.flex-1.overflow-hidden (hit-test miss)
- 🟡 `deep-chat-right-panel-literature` [mobile] `a.wrap-anywhere.font-medium` — interactive <a> "link" occluded at center by div.flex-1.overflow-hidden (hit-test miss)
- 🟡 `deep-chat-right-panel-literature` [tablet] `[data-testid="chat-input-add-btn"]` — interactive <button> "" occluded at center by [data-testid="ullm-model-select"] (hit-test miss)
- 🟡 `deep-chat-right-panel-multi` [desktop] `a.wrap-anywhere.font-medium` — interactive <a> "link" occluded at center by div (hit-test miss)
- … +129 more (see JSONL)

### I4 (6)

- 🟡 `deep-chat-right-panel-file` [mobile] `body` — overlay open but body scroll NOT locked
- 🟡 `deep-chat-right-panel-literature` [mobile] `body` — overlay open but body scroll NOT locked
- 🟡 `deep-chat-right-panel-multi` [mobile] `body` — overlay open but body scroll NOT locked
- 🟡 `deep-chat-streaming` [desktop] `body` — overlay open but body scroll NOT locked
- 🟡 `deep-chat-streaming` [mobile] `body` — overlay open but body scroll NOT locked
- 🟡 `deep-chat-streaming` [tablet] `body` — overlay open but body scroll NOT locked

### I5 (3)

- 🟡 `seeded-defect-repro` [desktop] `[data-testid="repro-i5-tablist"]` — horizontal strip has VERTICAL scroll (scrollHeight 84 > clientHeight 26, overflow-y:auto)
- 🟡 `seeded-defect-repro` [mobile] `[data-testid="repro-i5-tablist"]` — horizontal strip has VERTICAL scroll (scrollHeight 84 > clientHeight 26, overflow-y:auto)
- 🟡 `seeded-defect-repro` [tablet] `[data-testid="repro-i5-tablist"]` — horizontal strip has VERTICAL scroll (scrollHeight 84 > clientHeight 26, overflow-y:auto)

### J6 (10)

- 🟡 `chats` [mobile] `div.flex.items-center` — peer icon-only action group mixes button variants: {outline, default} — chat-history-search-toggle-btn=outline, chat-history-header-new-chat-btn=default
- 🟡 `chats` [mobile] `div.flex.items-center` — peer icon-only action group mixes button variants: {outline, default} — chat-history-search-toggle-btn=outline, chat-history-header-new-chat-btn=default
- 🟡 `chats` [mobile] `div.flex.items-center` — peer icon-only action group mixes button variants: {outline, default} — chat-history-search-toggle-btn=outline, chat-history-header-new-chat-btn=default
- 🟡 `seeded-chat-history-list` [mobile] `div.flex.items-center` — peer icon-only action group mixes button variants: {outline, default} — chat-history-search-toggle-btn=outline, chat-history-header-new-chat-btn=default
- 🟡 `seeded-defect-repro` [desktop] `[data-testid="repro-j6-group"]` — peer icon-only action group mixes button variants: {outline, ghost} — download=outline, open=ghost
- 🟡 `seeded-defect-repro` [mobile] `[data-testid="repro-j6-group"]` — peer icon-only action group mixes button variants: {outline, ghost} — download=outline, open=ghost
- 🟡 `seeded-defect-repro` [tablet] `[data-testid="repro-j6-group"]` — peer icon-only action group mixes button variants: {outline, ghost} — download=outline, open=ghost
- 🟡 `seeded-interact-provider-header` [desktop] `[data-testid="llm-provider-header-name-form"]` — peer icon-only action group mixes button variants: {default, outline} — llm-provider-header-save-name-btn=default, llm-provider-header-cancel-name-btn=outline
- 🟡 `seeded-interact-provider-header` [mobile] `[data-testid="llm-provider-header-name-form"]` — peer icon-only action group mixes button variants: {default, outline} — llm-provider-header-save-name-btn=default, llm-provider-header-cancel-name-btn=outline
- 🟡 `seeded-interact-provider-header` [tablet] `[data-testid="llm-provider-header-name-form"]` — peer icon-only action group mixes button variants: {default, outline} — llm-provider-header-save-name-btn=default, llm-provider-header-cancel-name-btn=outline

### J7 (188)

- 🟡 `chat` [desktop] `[data-testid="mcp-chip-e0000000-0000-0000-0000-0000000000e1-close"]` — "close" control on the right here but left in the majority of containers (107 left / 96 right) — inconsistent placement
- 🟡 `chat` [mobile] `[data-testid="mcp-chip-e0000000-0000-0000-0000-0000000000e1-close"]` — "close" control on the right here but left in the majority of containers (107 left / 96 right) — inconsistent placement
- 🟡 `chat` [tablet] `[data-testid="mcp-chip-e0000000-0000-0000-0000-0000000000e1-close"]` — "close" control on the right here but left in the majority of containers (107 left / 96 right) — inconsistent placement
- 🟡 `deep-chat-attachments` [desktop] `[data-testid="mcp-chip-e0000000-0000-0000-0000-0000000000e1-close"]` — "close" control on the right here but left in the majority of containers (107 left / 96 right) — inconsistent placement
- 🟡 `deep-chat-attachments` [mobile] `[data-testid="mcp-chip-e0000000-0000-0000-0000-0000000000e1-close"]` — "close" control on the right here but left in the majority of containers (107 left / 96 right) — inconsistent placement
- 🟡 `deep-chat-attachments` [tablet] `[data-testid="mcp-chip-e0000000-0000-0000-0000-0000000000e1-close"]` — "close" control on the right here but left in the majority of containers (107 left / 96 right) — inconsistent placement
- 🟡 `deep-chat-branched` [desktop] `[data-testid="mcp-chip-e0000000-0000-0000-0000-0000000000e1-close"]` — "close" control on the right here but left in the majority of containers (107 left / 96 right) — inconsistent placement
- 🟡 `deep-chat-branched` [mobile] `[data-testid="mcp-chip-e0000000-0000-0000-0000-0000000000e1-close"]` — "close" control on the right here but left in the majority of containers (107 left / 96 right) — inconsistent placement
- 🟡 `deep-chat-branched` [tablet] `[data-testid="mcp-chip-e0000000-0000-0000-0000-0000000000e1-close"]` — "close" control on the right here but left in the majority of containers (107 left / 96 right) — inconsistent placement
- 🟡 `deep-chat-elicitation` [desktop] `[data-testid="mcp-chip-e0000000-0000-0000-0000-0000000000e1-close"]` — "close" control on the right here but left in the majority of containers (107 left / 96 right) — inconsistent placement
- 🟡 `deep-chat-elicitation` [mobile] `[data-testid="mcp-chip-e0000000-0000-0000-0000-0000000000e1-close"]` — "close" control on the right here but left in the majority of containers (107 left / 96 right) — inconsistent placement
- 🟡 `deep-chat-elicitation` [tablet] `[data-testid="mcp-chip-e0000000-0000-0000-0000-0000000000e1-close"]` — "close" control on the right here but left in the majority of containers (107 left / 96 right) — inconsistent placement
- 🟡 `deep-chat-long` [desktop] `[data-testid="html-block-copy-btn"]` — "copy" control on the right here but left in the majority of containers (1750 left / 83 right) — inconsistent placement
- 🟡 `deep-chat-long` [desktop] `[data-testid="html-block-copy-btn"]` — "copy" control on the right here but left in the majority of containers (1750 left / 83 right) — inconsistent placement
- 🟡 `deep-chat-long` [desktop] `[data-testid="html-block-copy-btn"]` — "copy" control on the right here but left in the majority of containers (1750 left / 83 right) — inconsistent placement
- 🟡 `deep-chat-long` [mobile] `[data-testid="file-viewer-copy-btn"]` — "copy" control on the right here but left in the majority of containers (1750 left / 83 right) — inconsistent placement
- 🟡 `deep-chat-long` [mobile] `[data-testid="file-viewer-copy-btn"]` — "copy" control on the right here but left in the majority of containers (1750 left / 83 right) — inconsistent placement
- 🟡 `deep-chat-long` [mobile] `[data-testid="file-viewer-copy-btn"]` — "copy" control on the right here but left in the majority of containers (1750 left / 83 right) — inconsistent placement
- 🟡 `deep-chat-long` [mobile] `[data-testid="html-block-copy-btn"]` — "copy" control on the right here but left in the majority of containers (1750 left / 83 right) — inconsistent placement
- 🟡 `deep-chat-long` [mobile] `[data-testid="html-block-copy-btn"]` — "copy" control on the right here but left in the majority of containers (1750 left / 83 right) — inconsistent placement
- 🟡 `deep-chat-long` [mobile] `[data-testid="html-block-copy-btn"]` — "copy" control on the right here but left in the majority of containers (1750 left / 83 right) — inconsistent placement
- 🟡 `deep-chat-long` [mobile] `[data-testid="mcp-chip-e0000000-0000-0000-0000-0000000000e1-close"]` — "close" control on the right here but left in the majority of containers (107 left / 96 right) — inconsistent placement
- 🟡 `deep-chat-long` [tablet] `[data-testid="html-block-copy-btn"]` — "copy" control on the right here but left in the majority of containers (1750 left / 83 right) — inconsistent placement
- 🟡 `deep-chat-long` [tablet] `[data-testid="html-block-copy-btn"]` — "copy" control on the right here but left in the majority of containers (1750 left / 83 right) — inconsistent placement
- 🟡 `deep-chat-long` [tablet] `[data-testid="html-block-copy-btn"]` — "copy" control on the right here but left in the majority of containers (1750 left / 83 right) — inconsistent placement
- 🟡 `deep-chat-long` [tablet] `[data-testid="mcp-chip-e0000000-0000-0000-0000-0000000000e1-close"]` — "close" control on the right here but left in the majority of containers (107 left / 96 right) — inconsistent placement
- 🟡 `deep-chat-long` [tablet] `[data-testid="mcp-chip-e0000000-0000-0000-0000-0000000000e1-close"]` — "close" control on the right here but left in the majority of containers (107 left / 96 right) — inconsistent placement
- 🟡 `deep-chat-mcp-toolcall-completed` [desktop] `[data-testid="mcp-chip-e0000000-0000-0000-0000-0000000000e1-close"]` — "close" control on the right here but left in the majority of containers (107 left / 96 right) — inconsistent placement
- 🟡 `deep-chat-mcp-toolcall-completed` [mobile] `[data-testid="mcp-chip-e0000000-0000-0000-0000-0000000000e1-close"]` — "close" control on the right here but left in the majority of containers (107 left / 96 right) — inconsistent placement
- 🟡 `deep-chat-mcp-toolcall-completed` [tablet] `[data-testid="mcp-chip-e0000000-0000-0000-0000-0000000000e1-close"]` — "close" control on the right here but left in the majority of containers (107 left / 96 right) — inconsistent placement
- … +158 more (see JSONL)

### K1 (3)

- 🟡 `seeded-defect-repro` [desktop] `[data-testid="conversation-title"]` — persistent context "conversation-title" is a DESCENDANT of scroll container [data-testid="repro-k1-scroller"] — scrolls out of view (should be pinned chrome)
- 🟡 `seeded-defect-repro` [mobile] `[data-testid="conversation-title"]` — persistent context "conversation-title" is a DESCENDANT of scroll container [data-testid="repro-k1-scroller"] — scrolls out of view (should be pinned chrome)
- 🟡 `seeded-defect-repro` [tablet] `[data-testid="conversation-title"]` — persistent context "conversation-title" is a DESCENDANT of scroll container [data-testid="repro-k1-scroller"] — scrolls out of view (should be pinned chrome)

### L2 (3)

- 🟡 `seeded-defect-repro` [desktop] `pre` — mermaid source ("graph TD;
  A[Start] -->") did not render to <svg>
- 🟡 `seeded-defect-repro` [mobile] `pre` — mermaid source ("graph TD;
  A[Start] -->") did not render to <svg>
- 🟡 `seeded-defect-repro` [tablet] `pre` — mermaid source ("graph TD;
  A[Start] -->") did not render to <svg>

### L3 (39)

- 🟡 `deep-chat-long` [desktop] `code.language-html` — language-tagged code block has 0 token spans / 0 colors — highlighting not applied (single-color plaintext) [dev-serve]
- 🟡 `deep-chat-long` [desktop] `code.language-html` — language-tagged code block has 0 token spans / 0 colors — highlighting not applied (single-color plaintext) [dev-serve]
- 🟡 `deep-chat-long` [desktop] `pre.language-rust.bg-[var(--sdm-bg,inherit]` — language-tagged code block has 8 token spans / 1 colors — highlighting not applied (single-color plaintext) [dev-serve]
- 🟡 `deep-chat-long` [mobile] `code.language-html` — language-tagged code block has 0 token spans / 0 colors — highlighting not applied (single-color plaintext) [dev-serve]
- 🟡 `deep-chat-long` [mobile] `code.language-html` — language-tagged code block has 0 token spans / 0 colors — highlighting not applied (single-color plaintext) [dev-serve]
- 🟡 `deep-chat-long` [mobile] `pre.language-rust.bg-[var(--sdm-bg,inherit]` — language-tagged code block has 8 token spans / 1 colors — highlighting not applied (single-color plaintext) [dev-serve]
- 🟡 `deep-chat-long` [tablet] `code.language-html` — language-tagged code block has 0 token spans / 0 colors — highlighting not applied (single-color plaintext) [dev-serve]
- 🟡 `deep-chat-long` [tablet] `code.language-html` — language-tagged code block has 0 token spans / 0 colors — highlighting not applied (single-color plaintext) [dev-serve]
- 🟡 `deep-chat-long` [tablet] `pre.language-rust.bg-[var(--sdm-bg,inherit]` — language-tagged code block has 8 token spans / 1 colors — highlighting not applied (single-color plaintext) [dev-serve]
- 🟡 `deep-chat-no-models` [desktop] `code.language-html` — language-tagged code block has 0 token spans / 0 colors — highlighting not applied (single-color plaintext) [dev-serve]
- 🟡 `deep-chat-no-models` [desktop] `pre.language-rust.bg-[var(--sdm-bg,inherit]` — language-tagged code block has 8 token spans / 1 colors — highlighting not applied (single-color plaintext) [dev-serve]
- 🟡 `deep-chat-no-models` [mobile] `code.language-html` — language-tagged code block has 0 token spans / 0 colors — highlighting not applied (single-color plaintext) [dev-serve]
- 🟡 `deep-chat-no-models` [mobile] `pre.language-rust.bg-[var(--sdm-bg,inherit]` — language-tagged code block has 8 token spans / 1 colors — highlighting not applied (single-color plaintext) [dev-serve]
- 🟡 `deep-chat-no-models` [tablet] `code.language-html` — language-tagged code block has 0 token spans / 0 colors — highlighting not applied (single-color plaintext) [dev-serve]
- 🟡 `deep-chat-no-models` [tablet] `pre.language-rust.bg-[var(--sdm-bg,inherit]` — language-tagged code block has 8 token spans / 1 colors — highlighting not applied (single-color plaintext) [dev-serve]
- 🟡 `deep-chat-rendering-showcase` [desktop] `code.language-html` — language-tagged code block has 0 token spans / 0 colors — highlighting not applied (single-color plaintext) [dev-serve]
- 🟡 `deep-chat-rendering-showcase` [mobile] `code.language-html` — language-tagged code block has 0 token spans / 0 colors — highlighting not applied (single-color plaintext) [dev-serve]
- 🟡 `deep-chat-rendering-showcase` [tablet] `code.language-html` — language-tagged code block has 0 token spans / 0 colors — highlighting not applied (single-color plaintext) [dev-serve]
- 🟡 `deep-chat-right-panel-file` [desktop] `pre.language-rust.bg-[var(--sdm-bg,inherit]` — language-tagged code block has 8 token spans / 1 colors — highlighting not applied (single-color plaintext) [dev-serve]
- 🟡 `deep-chat-right-panel-file` [mobile] `pre.language-rust.bg-[var(--sdm-bg,inherit]` — language-tagged code block has 8 token spans / 1 colors — highlighting not applied (single-color plaintext) [dev-serve]
- 🟡 `deep-chat-right-panel-file` [tablet] `pre.language-rust.bg-[var(--sdm-bg,inherit]` — language-tagged code block has 8 token spans / 1 colors — highlighting not applied (single-color plaintext) [dev-serve]
- 🟡 `deep-chat-right-panel-literature` [desktop] `pre.language-rust.bg-[var(--sdm-bg,inherit]` — language-tagged code block has 8 token spans / 1 colors — highlighting not applied (single-color plaintext) [dev-serve]
- 🟡 `deep-chat-right-panel-literature` [mobile] `pre.language-rust.bg-[var(--sdm-bg,inherit]` — language-tagged code block has 8 token spans / 1 colors — highlighting not applied (single-color plaintext) [dev-serve]
- 🟡 `deep-chat-right-panel-literature` [tablet] `pre.language-rust.bg-[var(--sdm-bg,inherit]` — language-tagged code block has 8 token spans / 1 colors — highlighting not applied (single-color plaintext) [dev-serve]
- 🟡 `deep-chat-right-panel-multi` [desktop] `pre.language-rust.bg-[var(--sdm-bg,inherit]` — language-tagged code block has 8 token spans / 1 colors — highlighting not applied (single-color plaintext) [dev-serve]
- 🟡 `deep-chat-right-panel-multi` [mobile] `pre.language-rust.bg-[var(--sdm-bg,inherit]` — language-tagged code block has 8 token spans / 1 colors — highlighting not applied (single-color plaintext) [dev-serve]
- 🟡 `deep-chat-right-panel-multi` [tablet] `pre.language-rust.bg-[var(--sdm-bg,inherit]` — language-tagged code block has 8 token spans / 1 colors — highlighting not applied (single-color plaintext) [dev-serve]
- 🟡 `deep-chat-streaming` [desktop] `code.language-html` — language-tagged code block has 0 token spans / 0 colors — highlighting not applied (single-color plaintext) [dev-serve]
- 🟡 `deep-chat-streaming` [desktop] `pre.language-rust.bg-[var(--sdm-bg,inherit]` — language-tagged code block has 8 token spans / 1 colors — highlighting not applied (single-color plaintext) [dev-serve]
- 🟡 `deep-chat-streaming` [mobile] `code.language-html` — language-tagged code block has 0 token spans / 0 colors — highlighting not applied (single-color plaintext) [dev-serve]
- … +9 more (see JSONL)
