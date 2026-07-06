# Geometry findings (GENERATED)

> `node scripts/gallery-geometry-audit.mjs` (Layer 1 — see `docs/DEFECT_TAXONOMY.md`). Deterministic DOM-geometry rules over every gallery surface × state × viewport. Each row cites surface, viewport, taxonomy class, selector, and measured numbers.

## Totals

| Severity | Count |
|---|---|
| 🔴 HIGH (gating) | 0 |
| 🔵 HIGH (allow-listed) | 3 |
| 🟡 MEDIUM | 2636 |
| ⚪ LOW | 1533 |
| **Total** | **4172** |

## By taxonomy class

| Class | HIGH | MEDIUM | LOW | allow-listed |
|---|---|---|---|---|
| A1 | 3 | 0 | 0 | 3 |
| A2 | 0 | 49 | 0 | 0 |
| A3 | 0 | 862 | 0 | 0 |
| A4 | 0 | 0 | 15 | 0 |
| A7 | 0 | 0 | 274 | 0 |
| A9 | 0 | 203 | 0 | 0 |
| B1 | 0 | 20 | 0 | 0 |
| B2 | 0 | 3 | 0 | 0 |
| B8 | 0 | 0 | 27 | 0 |
| C10 | 0 | 9 | 0 | 0 |
| C12 | 0 | 351 | 27 | 0 |
| C7 | 0 | 45 | 0 | 0 |
| D1 | 0 | 212 | 0 | 0 |
| G5 | 0 | 474 | 1190 | 0 |
| G7 | 0 | 143 | 0 | 0 |
| I1 | 0 | 137 | 0 | 0 |
| I4 | 0 | 3 | 0 | 0 |
| J6 | 0 | 4 | 0 | 0 |
| J7 | 0 | 65 | 0 | 0 |
| K1 | 0 | 14 | 0 | 0 |
| L2 | 0 | 21 | 0 | 0 |
| L3 | 0 | 21 | 0 | 0 |

## Gating HIGH findings (0)

_None — geometry is clean of un-allow-listed HIGH findings._

## MEDIUM findings (2636)

### A2 (49)

- 🟡 `deep-chat-long` [mobile] `code.rounded.bg-muted ∩ a.wrap-anywhere.font-medium` — sibling boxes overlap by 538px² (100% of smaller)
- 🟡 `deep-chat-long` [mobile] `del ∩ code.rounded.bg-muted` — sibling boxes overlap by 1927px² (95% of smaller)
- 🟡 `deep-chat-long` [mobile] `em ∩ code.rounded.bg-muted` — sibling boxes overlap by 4108px² (41% of smaller)
- 🟡 `deep-chat-long` [mobile] `em ∩ del` — sibling boxes overlap by 2034px² (100% of smaller)
- 🟡 `deep-chat-long` [mobile] `em ∩ em` — sibling boxes overlap by 726px² (100% of smaller)
- 🟡 `deep-chat-long` [mobile] `span.font-semibold ∩ em` — sibling boxes overlap by 748px² (100% of smaller)
- 🟡 `deep-chat-right-panel-file` [mobile] `code.rounded.bg-muted ∩ a.wrap-anywhere.font-medium` — sibling boxes overlap by 538px² (100% of smaller)
- 🟡 `deep-chat-right-panel-file` [mobile] `del ∩ code.rounded.bg-muted` — sibling boxes overlap by 1927px² (95% of smaller)
- 🟡 `deep-chat-right-panel-file` [mobile] `em ∩ code.rounded.bg-muted` — sibling boxes overlap by 4108px² (41% of smaller)
- 🟡 `deep-chat-right-panel-file` [mobile] `em ∩ del` — sibling boxes overlap by 2034px² (100% of smaller)
- 🟡 `deep-chat-right-panel-file` [mobile] `em ∩ em` — sibling boxes overlap by 726px² (100% of smaller)
- 🟡 `deep-chat-right-panel-file` [mobile] `span.font-semibold ∩ em` — sibling boxes overlap by 748px² (100% of smaller)
- 🟡 `deep-chat-right-panel-file` [tablet] `span.text-sm.font-semibold ∩ span.text-muted-foreground.ml-2` — sibling boxes overlap by 1575px² (88% of smaller)
- 🟡 `deep-chat-right-panel-file` [tablet] `span.text-sm.font-semibold ∩ span.text-muted-foreground.ml-2` — sibling boxes overlap by 1575px² (88% of smaller)
- 🟡 `deep-chat-right-panel-file` [tablet] `span.text-sm.font-semibold ∩ span.text-muted-foreground.ml-2` — sibling boxes overlap by 1575px² (88% of smaller)
- 🟡 `deep-chat-right-panel-literature` [mobile] `code.rounded.bg-muted ∩ a.wrap-anywhere.font-medium` — sibling boxes overlap by 538px² (100% of smaller)
- 🟡 `deep-chat-right-panel-literature` [mobile] `del ∩ code.rounded.bg-muted` — sibling boxes overlap by 1927px² (95% of smaller)
- 🟡 `deep-chat-right-panel-literature` [mobile] `em ∩ code.rounded.bg-muted` — sibling boxes overlap by 4108px² (41% of smaller)
- 🟡 `deep-chat-right-panel-literature` [mobile] `em ∩ del` — sibling boxes overlap by 2034px² (100% of smaller)
- 🟡 `deep-chat-right-panel-literature` [mobile] `em ∩ em` — sibling boxes overlap by 726px² (100% of smaller)
- 🟡 `deep-chat-right-panel-literature` [mobile] `span.font-semibold ∩ em` — sibling boxes overlap by 748px² (100% of smaller)
- 🟡 `deep-chat-right-panel-literature` [tablet] `span.text-sm.font-semibold ∩ span.text-muted-foreground.ml-2` — sibling boxes overlap by 1575px² (88% of smaller)
- 🟡 `deep-chat-right-panel-literature` [tablet] `span.text-sm.font-semibold ∩ span.text-muted-foreground.ml-2` — sibling boxes overlap by 1575px² (88% of smaller)
- 🟡 `deep-chat-right-panel-literature` [tablet] `span.text-sm.font-semibold ∩ span.text-muted-foreground.ml-2` — sibling boxes overlap by 1575px² (88% of smaller)
- 🟡 `deep-chat-right-panel-multi` [mobile] `code.rounded.bg-muted ∩ a.wrap-anywhere.font-medium` — sibling boxes overlap by 538px² (100% of smaller)
- 🟡 `deep-chat-right-panel-multi` [mobile] `del ∩ code.rounded.bg-muted` — sibling boxes overlap by 1927px² (95% of smaller)
- 🟡 `deep-chat-right-panel-multi` [mobile] `em ∩ code.rounded.bg-muted` — sibling boxes overlap by 4108px² (41% of smaller)
- 🟡 `deep-chat-right-panel-multi` [mobile] `em ∩ del` — sibling boxes overlap by 2034px² (100% of smaller)
- 🟡 `deep-chat-right-panel-multi` [mobile] `em ∩ em` — sibling boxes overlap by 726px² (100% of smaller)
- 🟡 `deep-chat-right-panel-multi` [mobile] `span.font-semibold ∩ em` — sibling boxes overlap by 748px² (100% of smaller)
- … +19 more (see JSONL)

### A3 (862)

- 🟡 `chat` [desktop] `span.[&_svg]:size-4` — protrudes 3px past parent [data-testid="ullm-model-retry"] (no overflow clip)
- 🟡 `chat` [mobile] `span.[&_svg]:size-4` — protrudes 3px past parent [data-testid="ullm-model-retry"] (no overflow clip)
- 🟡 `chat` [mobile] `span#base-ui-_r_a_[data-slot=tooltip-trigger].inline-flex.shrink-0` — protrudes 28px past parent div.flex.items-center (no overflow clip)
- 🟡 `chat` [tablet] `span.[&_svg]:size-4` — protrudes 3px past parent [data-testid="ullm-model-retry"] (no overflow clip)
- 🟡 `deep-chat-attachments` [mobile] `[data-testid="chat-export-btn"]` — protrudes 60px past parent div.flex.items-center (no overflow clip)
- 🟡 `deep-chat-branched` [mobile] `[data-testid="chat-export-btn"]` — protrudes 60px past parent div.flex.items-center (no overflow clip)
- 🟡 `deep-chat-elicitation` [mobile] `[data-testid="chat-export-btn"]` — protrudes 60px past parent div.flex.items-center (no overflow clip)
- 🟡 `deep-chat-long` [mobile] `[data-testid="chat-export-btn"]` — protrudes 60px past parent div.flex.items-center (no overflow clip)
- 🟡 `deep-chat-long` [mobile] `[data-testid="inline-file-preview-open-panel"]` — protrudes 12px past parent div.flex.items-center (no overflow clip)
- 🟡 `deep-chat-long` [mobile] `[data-testid="inline-file-preview-open-panel"]` — protrudes 12px past parent div.flex.items-center (no overflow clip)
- 🟡 `deep-chat-long` [mobile] `[data-testid="inline-file-preview-open-panel"]` — protrudes 12px past parent div.flex.items-center (no overflow clip)
- 🟡 `deep-chat-long` [mobile] `[data-testid="inline-file-preview-open"]` — protrudes 58px past parent div.flex.items-center (no overflow clip)
- 🟡 `deep-chat-long` [mobile] `[data-testid="inline-file-preview-open"]` — protrudes 58px past parent div.flex.items-center (no overflow clip)
- 🟡 `deep-chat-long` [mobile] `[data-testid="inline-file-preview-open"]` — protrudes 40px past parent div.flex.items-center (no overflow clip)
- 🟡 `deep-chat-long` [mobile] `[data-testid="inline-file-preview-open"]` — protrudes 10px past parent div.flex.items-center (no overflow clip)
- 🟡 `deep-chat-long` [mobile] `[data-testid="inline-file-preview-open"]` — protrudes 58px past parent div.flex.items-center (no overflow clip)
- 🟡 `deep-chat-long` [mobile] `span.text-[var(--sdm-c,inherit)].dark:text-[var(--shiki-dark,var(--sdm-c,inherit))]` — protrudes 28px past parent span.block.before:content-[counter(line)] (no overflow clip)
- 🟡 `deep-chat-long` [mobile] `span.text-[var(--sdm-c,inherit)].dark:text-[var(--shiki-dark,var(--sdm-c,inherit))]` — protrudes 11px past parent span.block.before:content-[counter(line)] (no overflow clip)
- 🟡 `deep-chat-long` [mobile] `span.text-[var(--sdm-c,inherit)].dark:text-[var(--shiki-dark,var(--sdm-c,inherit))]` — protrudes 45px past parent span.block.before:content-[counter(line)] (no overflow clip)
- 🟡 `deep-chat-long` [mobile] `span.text-[var(--sdm-c,inherit)].dark:text-[var(--shiki-dark,var(--sdm-c,inherit))]` — protrudes 36px past parent span.block.before:content-[counter(line)] (no overflow clip)
- 🟡 `deep-chat-long` [mobile] `span.text-[var(--sdm-c,inherit)].dark:text-[var(--shiki-dark,var(--sdm-c,inherit))]` — protrudes 70px past parent span.block.before:content-[counter(line)] (no overflow clip)
- 🟡 `deep-chat-long` [mobile] `span.text-[var(--sdm-c,inherit)].dark:text-[var(--shiki-dark,var(--sdm-c,inherit))]` — protrudes 112px past parent span.block.before:content-[counter(line)] (no overflow clip)
- 🟡 `deep-chat-long` [mobile] `span.text-[var(--sdm-c,inherit)].dark:text-[var(--shiki-dark,var(--sdm-c,inherit))]` — protrudes 20px past parent span.block.before:content-[counter(line)] (no overflow clip)
- 🟡 `deep-chat-long` [mobile] `span.text-[var(--sdm-c,inherit)].dark:text-[var(--shiki-dark,var(--sdm-c,inherit))]` — protrudes 204px past parent span.block.before:content-[counter(line)] (no overflow clip)
- 🟡 `deep-chat-long` [mobile] `span.text-[var(--sdm-c,inherit)].dark:text-[var(--shiki-dark,var(--sdm-c,inherit))]` — protrudes 20px past parent span.block.before:content-[counter(line)] (no overflow clip)
- 🟡 `deep-chat-long` [mobile] `span.text-[var(--sdm-c,inherit)].dark:text-[var(--shiki-dark,var(--sdm-c,inherit))]` — protrudes 20px past parent span.block.before:content-[counter(line)] (no overflow clip)
- 🟡 `deep-chat-long` [mobile] `span.text-[var(--sdm-c,inherit)].dark:text-[var(--shiki-dark,var(--sdm-c,inherit))]` — protrudes 20px past parent span.block.before:content-[counter(line)] (no overflow clip)
- 🟡 `deep-chat-long` [mobile] `span.text-[var(--sdm-c,inherit)].dark:text-[var(--shiki-dark,var(--sdm-c,inherit))]` — protrudes 20px past parent span.block.before:content-[counter(line)] (no overflow clip)
- 🟡 `deep-chat-long` [mobile] `span.text-[var(--sdm-c,inherit)].dark:text-[var(--shiki-dark,var(--sdm-c,inherit))]` — protrudes 20px past parent span.block.before:content-[counter(line)] (no overflow clip)
- 🟡 `deep-chat-long` [mobile] `span.text-[var(--sdm-c,inherit)].dark:text-[var(--shiki-dark,var(--sdm-c,inherit))]` — protrudes 20px past parent span.block.before:content-[counter(line)] (no overflow clip)
- … +832 more (see JSONL)

### A9 (203)

- 🟡 `deep-chat-long` [desktop] `[data-testid="inline-file-preview"]` — peer metric mismatch (icon-height): 24px vs group mode 16px among 4 same-kind siblings
- 🟡 `deep-chat-long` [desktop] `[data-testid="inline-file-preview"]` — peer metric mismatch (icon-height): 24px vs group mode 16px among 4 same-kind siblings
- 🟡 `deep-chat-long` [desktop] `[data-testid="inline-file-preview"]` — peer metric mismatch (icon-width): 20px vs group mode 16px among 4 same-kind siblings
- 🟡 `deep-chat-long` [desktop] `[data-testid="inline-file-preview"]` — peer metric mismatch (icon-width): 20px vs group mode 16px among 4 same-kind siblings
- 🟡 `deep-chat-long` [mobile] `[data-testid="inline-file-preview"]` — peer metric mismatch (icon-height): 24px vs group mode 16px among 4 same-kind siblings
- 🟡 `deep-chat-long` [mobile] `[data-testid="inline-file-preview"]` — peer metric mismatch (icon-height): 24px vs group mode 16px among 4 same-kind siblings
- 🟡 `deep-chat-long` [mobile] `[data-testid="inline-file-preview"]` — peer metric mismatch (icon-width): 20px vs group mode 16px among 4 same-kind siblings
- 🟡 `deep-chat-long` [mobile] `[data-testid="inline-file-preview"]` — peer metric mismatch (icon-width): 20px vs group mode 16px among 4 same-kind siblings
- 🟡 `deep-chat-long` [tablet] `[data-testid="inline-file-preview"]` — peer metric mismatch (icon-height): 24px vs group mode 16px among 4 same-kind siblings
- 🟡 `deep-chat-long` [tablet] `[data-testid="inline-file-preview"]` — peer metric mismatch (icon-height): 24px vs group mode 16px among 4 same-kind siblings
- 🟡 `deep-chat-long` [tablet] `[data-testid="inline-file-preview"]` — peer metric mismatch (icon-width): 20px vs group mode 16px among 4 same-kind siblings
- 🟡 `deep-chat-long` [tablet] `[data-testid="inline-file-preview"]` — peer metric mismatch (icon-width): 20px vs group mode 16px among 4 same-kind siblings
- 🟡 `deep-chat-right-panel-file` [desktop] `[data-testid="inline-file-preview"]` — peer metric mismatch (icon-height): 24px vs group mode 16px among 4 same-kind siblings
- 🟡 `deep-chat-right-panel-file` [desktop] `[data-testid="inline-file-preview"]` — peer metric mismatch (icon-height): 24px vs group mode 16px among 4 same-kind siblings
- 🟡 `deep-chat-right-panel-file` [desktop] `[data-testid="inline-file-preview"]` — peer metric mismatch (icon-width): 20px vs group mode 16px among 4 same-kind siblings
- 🟡 `deep-chat-right-panel-file` [desktop] `[data-testid="inline-file-preview"]` — peer metric mismatch (icon-width): 20px vs group mode 16px among 4 same-kind siblings
- 🟡 `deep-chat-right-panel-file` [mobile] `[data-testid="inline-file-preview"]` — peer metric mismatch (icon-height): 24px vs group mode 16px among 4 same-kind siblings
- 🟡 `deep-chat-right-panel-file` [mobile] `[data-testid="inline-file-preview"]` — peer metric mismatch (icon-height): 24px vs group mode 16px among 4 same-kind siblings
- 🟡 `deep-chat-right-panel-file` [mobile] `[data-testid="inline-file-preview"]` — peer metric mismatch (icon-width): 20px vs group mode 16px among 4 same-kind siblings
- 🟡 `deep-chat-right-panel-file` [mobile] `[data-testid="inline-file-preview"]` — peer metric mismatch (icon-width): 20px vs group mode 16px among 4 same-kind siblings
- 🟡 `deep-chat-right-panel-file` [tablet] `[data-testid="inline-file-preview"]` — peer metric mismatch (icon-height): 24px vs group mode 16px among 4 same-kind siblings
- 🟡 `deep-chat-right-panel-file` [tablet] `[data-testid="inline-file-preview"]` — peer metric mismatch (icon-height): 24px vs group mode 16px among 4 same-kind siblings
- 🟡 `deep-chat-right-panel-file` [tablet] `[data-testid="inline-file-preview"]` — peer metric mismatch (icon-width): 20px vs group mode 16px among 4 same-kind siblings
- 🟡 `deep-chat-right-panel-file` [tablet] `[data-testid="inline-file-preview"]` — peer metric mismatch (icon-width): 20px vs group mode 16px among 4 same-kind siblings
- 🟡 `deep-chat-right-panel-literature` [desktop] `[data-testid="inline-file-preview"]` — peer metric mismatch (icon-height): 24px vs group mode 16px among 4 same-kind siblings
- 🟡 `deep-chat-right-panel-literature` [desktop] `[data-testid="inline-file-preview"]` — peer metric mismatch (icon-height): 24px vs group mode 16px among 4 same-kind siblings
- 🟡 `deep-chat-right-panel-literature` [desktop] `[data-testid="inline-file-preview"]` — peer metric mismatch (icon-width): 20px vs group mode 16px among 4 same-kind siblings
- 🟡 `deep-chat-right-panel-literature` [desktop] `[data-testid="inline-file-preview"]` — peer metric mismatch (icon-width): 20px vs group mode 16px among 4 same-kind siblings
- 🟡 `deep-chat-right-panel-literature` [mobile] `[data-testid="inline-file-preview"]` — peer metric mismatch (icon-height): 24px vs group mode 16px among 4 same-kind siblings
- 🟡 `deep-chat-right-panel-literature` [mobile] `[data-testid="inline-file-preview"]` — peer metric mismatch (icon-height): 24px vs group mode 16px among 4 same-kind siblings
- … +173 more (see JSONL)

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

### B2 (3)

- 🟡 `chat` [desktop] `[data-testid="ullm-model-retry"]` — nowrap text overflows by 4px without wrap or ellipsis
- 🟡 `chat` [mobile] `[data-testid="ullm-model-retry"]` — nowrap text overflows by 4px without wrap or ellipsis
- 🟡 `chat` [tablet] `[data-testid="ullm-model-retry"]` — nowrap text overflows by 4px without wrap or ellipsis

### C10 (9)

- 🟡 `chats` [desktop] `svg` — icon height 64px is 2.00× the adjacent text line-height 32px (oversized)
- 🟡 `chats` [mobile] `svg` — icon height 64px is 2.00× the adjacent text line-height 32px (oversized)
- 🟡 `chats` [tablet] `svg` — icon height 64px is 2.00× the adjacent text line-height 32px (oversized)
- 🟡 `deep-project-detail-error` [desktop] `svg` — icon height 48px is 1.71× the adjacent text line-height 28px (oversized)
- 🟡 `deep-project-detail-error` [mobile] `svg` — icon height 48px is 1.71× the adjacent text line-height 28px (oversized)
- 🟡 `deep-project-detail-error` [tablet] `svg` — icon height 48px is 1.71× the adjacent text line-height 28px (oversized)
- 🟡 `projects` [desktop] `svg` — icon height 64px is 2.00× the adjacent text line-height 32px (oversized)
- 🟡 `projects` [mobile] `svg` — icon height 64px is 2.00× the adjacent text line-height 32px (oversized)
- 🟡 `projects` [tablet] `svg` — icon height 64px is 2.00× the adjacent text line-height 32px (oversized)

### C12 (351)

- 🟡 `deep-chat-attachments` [desktop] `span[data-slot=avatar-fallback].flex.size-full` — bare placeholder circle: rounded-full 32×32px with no img/svg/initials content
- 🟡 `deep-chat-attachments` [mobile] `span[data-slot=avatar-fallback].flex.size-full` — bare placeholder circle: rounded-full 32×32px with no img/svg/initials content
- 🟡 `deep-chat-attachments` [tablet] `span[data-slot=avatar-fallback].flex.size-full` — bare placeholder circle: rounded-full 32×32px with no img/svg/initials content
- 🟡 `deep-chat-branched` [desktop] `span[data-slot=avatar-fallback].flex.size-full` — bare placeholder circle: rounded-full 32×32px with no img/svg/initials content
- 🟡 `deep-chat-branched` [mobile] `span[data-slot=avatar-fallback].flex.size-full` — bare placeholder circle: rounded-full 32×32px with no img/svg/initials content
- 🟡 `deep-chat-branched` [tablet] `span[data-slot=avatar-fallback].flex.size-full` — bare placeholder circle: rounded-full 32×32px with no img/svg/initials content
- 🟡 `deep-chat-elicitation` [desktop] `span[data-slot=avatar-fallback].flex.size-full` — bare placeholder circle: rounded-full 32×32px with no img/svg/initials content
- 🟡 `deep-chat-elicitation` [mobile] `span[data-slot=avatar-fallback].flex.size-full` — bare placeholder circle: rounded-full 32×32px with no img/svg/initials content
- 🟡 `deep-chat-elicitation` [tablet] `span[data-slot=avatar-fallback].flex.size-full` — bare placeholder circle: rounded-full 32×32px with no img/svg/initials content
- 🟡 `deep-chat-long` [desktop] `span[data-slot=avatar-fallback].flex.size-full` — bare placeholder circle: rounded-full 32×32px with no img/svg/initials content
- 🟡 `deep-chat-long` [desktop] `span[data-slot=avatar-fallback].flex.size-full` — bare placeholder circle: rounded-full 32×32px with no img/svg/initials content
- 🟡 `deep-chat-long` [desktop] `span[data-slot=avatar-fallback].flex.size-full` — bare placeholder circle: rounded-full 32×32px with no img/svg/initials content
- 🟡 `deep-chat-long` [desktop] `span[data-slot=avatar-fallback].flex.size-full` — bare placeholder circle: rounded-full 32×32px with no img/svg/initials content
- 🟡 `deep-chat-long` [desktop] `span[data-slot=avatar-fallback].flex.size-full` — bare placeholder circle: rounded-full 32×32px with no img/svg/initials content
- 🟡 `deep-chat-long` [desktop] `span[data-slot=avatar-fallback].flex.size-full` — bare placeholder circle: rounded-full 32×32px with no img/svg/initials content
- 🟡 `deep-chat-long` [desktop] `span[data-slot=avatar-fallback].flex.size-full` — bare placeholder circle: rounded-full 32×32px with no img/svg/initials content
- 🟡 `deep-chat-long` [desktop] `span[data-slot=avatar-fallback].flex.size-full` — bare placeholder circle: rounded-full 32×32px with no img/svg/initials content
- 🟡 `deep-chat-long` [desktop] `span[data-slot=avatar-fallback].flex.size-full` — bare placeholder circle: rounded-full 32×32px with no img/svg/initials content
- 🟡 `deep-chat-long` [desktop] `span[data-slot=avatar-fallback].flex.size-full` — bare placeholder circle: rounded-full 32×32px with no img/svg/initials content
- 🟡 `deep-chat-long` [desktop] `span[data-slot=avatar-fallback].flex.size-full` — bare placeholder circle: rounded-full 32×32px with no img/svg/initials content
- 🟡 `deep-chat-long` [desktop] `span[data-slot=avatar-fallback].flex.size-full` — bare placeholder circle: rounded-full 32×32px with no img/svg/initials content
- 🟡 `deep-chat-long` [desktop] `span[data-slot=avatar-fallback].flex.size-full` — bare placeholder circle: rounded-full 32×32px with no img/svg/initials content
- 🟡 `deep-chat-long` [desktop] `span[data-slot=avatar-fallback].flex.size-full` — bare placeholder circle: rounded-full 32×32px with no img/svg/initials content
- 🟡 `deep-chat-long` [desktop] `span[data-slot=avatar-fallback].flex.size-full` — bare placeholder circle: rounded-full 32×32px with no img/svg/initials content
- 🟡 `deep-chat-long` [desktop] `span[data-slot=avatar-fallback].flex.size-full` — bare placeholder circle: rounded-full 32×32px with no img/svg/initials content
- 🟡 `deep-chat-long` [desktop] `span[data-slot=avatar-fallback].flex.size-full` — bare placeholder circle: rounded-full 32×32px with no img/svg/initials content
- 🟡 `deep-chat-long` [desktop] `span[data-slot=avatar-fallback].flex.size-full` — bare placeholder circle: rounded-full 32×32px with no img/svg/initials content
- 🟡 `deep-chat-long` [mobile] `span[data-slot=avatar-fallback].flex.size-full` — bare placeholder circle: rounded-full 32×32px with no img/svg/initials content
- 🟡 `deep-chat-long` [mobile] `span[data-slot=avatar-fallback].flex.size-full` — bare placeholder circle: rounded-full 32×32px with no img/svg/initials content
- 🟡 `deep-chat-long` [mobile] `span[data-slot=avatar-fallback].flex.size-full` — bare placeholder circle: rounded-full 32×32px with no img/svg/initials content
- … +321 more (see JSONL)

### C7 (45)

- 🟡 `deep-chat-attachments` [desktop] `[data-role="user"] vs [data-role="assistant"]` — two DIFFERENT roles ("user" vs "assistant") render with an IDENTICAL visual signature (bg=32,32,32|align=center|border=0|avatar=0) — reader can't tell them apart
- 🟡 `deep-chat-attachments` [mobile] `[data-role="user"] vs [data-role="assistant"]` — two DIFFERENT roles ("user" vs "assistant") render with an IDENTICAL visual signature (bg=32,32,32|align=center|border=0|avatar=0) — reader can't tell them apart
- 🟡 `deep-chat-attachments` [tablet] `[data-role="user"] vs [data-role="assistant"]` — two DIFFERENT roles ("user" vs "assistant") render with an IDENTICAL visual signature (bg=32,32,32|align=center|border=0|avatar=0) — reader can't tell them apart
- 🟡 `deep-chat-branched` [desktop] `[data-role="user"] vs [data-role="assistant"]` — two DIFFERENT roles ("user" vs "assistant") render with an IDENTICAL visual signature (bg=32,32,32|align=center|border=0|avatar=0) — reader can't tell them apart
- 🟡 `deep-chat-branched` [mobile] `[data-role="user"] vs [data-role="assistant"]` — two DIFFERENT roles ("user" vs "assistant") render with an IDENTICAL visual signature (bg=32,32,32|align=center|border=0|avatar=0) — reader can't tell them apart
- 🟡 `deep-chat-branched` [tablet] `[data-role="user"] vs [data-role="assistant"]` — two DIFFERENT roles ("user" vs "assistant") render with an IDENTICAL visual signature (bg=32,32,32|align=center|border=0|avatar=0) — reader can't tell them apart
- 🟡 `deep-chat-elicitation` [desktop] `[data-role="user"] vs [data-role="assistant"]` — two DIFFERENT roles ("user" vs "assistant") render with an IDENTICAL visual signature (bg=32,32,32|align=center|border=0|avatar=0) — reader can't tell them apart
- 🟡 `deep-chat-elicitation` [mobile] `[data-role="user"] vs [data-role="assistant"]` — two DIFFERENT roles ("user" vs "assistant") render with an IDENTICAL visual signature (bg=32,32,32|align=center|border=0|avatar=0) — reader can't tell them apart
- 🟡 `deep-chat-elicitation` [tablet] `[data-role="user"] vs [data-role="assistant"]` — two DIFFERENT roles ("user" vs "assistant") render with an IDENTICAL visual signature (bg=32,32,32|align=center|border=0|avatar=0) — reader can't tell them apart
- 🟡 `deep-chat-long` [desktop] `[data-role="user"] vs [data-role="assistant"]` — two DIFFERENT roles ("user" vs "assistant") render with an IDENTICAL visual signature (bg=32,32,32|align=center|border=0|avatar=0) — reader can't tell them apart
- 🟡 `deep-chat-long` [mobile] `[data-role="user"] vs [data-role="assistant"]` — two DIFFERENT roles ("user" vs "assistant") render with an IDENTICAL visual signature (bg=32,32,32|align=center|border=0|avatar=0) — reader can't tell them apart
- 🟡 `deep-chat-long` [tablet] `[data-role="user"] vs [data-role="assistant"]` — two DIFFERENT roles ("user" vs "assistant") render with an IDENTICAL visual signature (bg=32,32,32|align=center|border=0|avatar=0) — reader can't tell them apart
- 🟡 `deep-chat-mcp-toolcall-completed` [desktop] `[data-role="user"] vs [data-role="assistant"]` — two DIFFERENT roles ("user" vs "assistant") render with an IDENTICAL visual signature (bg=32,32,32|align=center|border=0|avatar=0) — reader can't tell them apart
- 🟡 `deep-chat-mcp-toolcall-completed` [mobile] `[data-role="user"] vs [data-role="assistant"]` — two DIFFERENT roles ("user" vs "assistant") render with an IDENTICAL visual signature (bg=32,32,32|align=center|border=0|avatar=0) — reader can't tell them apart
- 🟡 `deep-chat-mcp-toolcall-completed` [tablet] `[data-role="user"] vs [data-role="assistant"]` — two DIFFERENT roles ("user" vs "assistant") render with an IDENTICAL visual signature (bg=32,32,32|align=center|border=0|avatar=0) — reader can't tell them apart
- 🟡 `deep-chat-mcp-toolcall-error` [desktop] `[data-role="user"] vs [data-role="assistant"]` — two DIFFERENT roles ("user" vs "assistant") render with an IDENTICAL visual signature (bg=32,32,32|align=center|border=0|avatar=0) — reader can't tell them apart
- 🟡 `deep-chat-mcp-toolcall-error` [mobile] `[data-role="user"] vs [data-role="assistant"]` — two DIFFERENT roles ("user" vs "assistant") render with an IDENTICAL visual signature (bg=32,32,32|align=center|border=0|avatar=0) — reader can't tell them apart
- 🟡 `deep-chat-mcp-toolcall-error` [tablet] `[data-role="user"] vs [data-role="assistant"]` — two DIFFERENT roles ("user" vs "assistant") render with an IDENTICAL visual signature (bg=32,32,32|align=center|border=0|avatar=0) — reader can't tell them apart
- 🟡 `deep-chat-rendering-showcase` [desktop] `[data-role="user"] vs [data-role="assistant"]` — two DIFFERENT roles ("user" vs "assistant") render with an IDENTICAL visual signature (bg=32,32,32|align=center|border=0|avatar=0) — reader can't tell them apart
- 🟡 `deep-chat-rendering-showcase` [mobile] `[data-role="user"] vs [data-role="assistant"]` — two DIFFERENT roles ("user" vs "assistant") render with an IDENTICAL visual signature (bg=32,32,32|align=center|border=0|avatar=0) — reader can't tell them apart
- 🟡 `deep-chat-rendering-showcase` [tablet] `[data-role="user"] vs [data-role="assistant"]` — two DIFFERENT roles ("user" vs "assistant") render with an IDENTICAL visual signature (bg=32,32,32|align=center|border=0|avatar=0) — reader can't tell them apart
- 🟡 `deep-chat-right-panel-file` [desktop] `[data-role="user"] vs [data-role="assistant"]` — two DIFFERENT roles ("user" vs "assistant") render with an IDENTICAL visual signature (bg=32,32,32|align=center|border=0|avatar=0) — reader can't tell them apart
- 🟡 `deep-chat-right-panel-file` [mobile] `[data-role="user"] vs [data-role="assistant"]` — two DIFFERENT roles ("user" vs "assistant") render with an IDENTICAL visual signature (bg=32,32,32|align=center|border=0|avatar=0) — reader can't tell them apart
- 🟡 `deep-chat-right-panel-file` [tablet] `[data-role="user"] vs [data-role="assistant"]` — two DIFFERENT roles ("user" vs "assistant") render with an IDENTICAL visual signature (bg=32,32,32|align=center|border=0|avatar=0) — reader can't tell them apart
- 🟡 `deep-chat-right-panel-literature` [desktop] `[data-role="user"] vs [data-role="assistant"]` — two DIFFERENT roles ("user" vs "assistant") render with an IDENTICAL visual signature (bg=32,32,32|align=center|border=0|avatar=0) — reader can't tell them apart
- 🟡 `deep-chat-right-panel-literature` [mobile] `[data-role="user"] vs [data-role="assistant"]` — two DIFFERENT roles ("user" vs "assistant") render with an IDENTICAL visual signature (bg=32,32,32|align=center|border=0|avatar=0) — reader can't tell them apart
- 🟡 `deep-chat-right-panel-literature` [tablet] `[data-role="user"] vs [data-role="assistant"]` — two DIFFERENT roles ("user" vs "assistant") render with an IDENTICAL visual signature (bg=32,32,32|align=center|border=0|avatar=0) — reader can't tell them apart
- 🟡 `deep-chat-right-panel-multi` [desktop] `[data-role="user"] vs [data-role="assistant"]` — two DIFFERENT roles ("user" vs "assistant") render with an IDENTICAL visual signature (bg=32,32,32|align=center|border=0|avatar=0) — reader can't tell them apart
- 🟡 `deep-chat-right-panel-multi` [mobile] `[data-role="user"] vs [data-role="assistant"]` — two DIFFERENT roles ("user" vs "assistant") render with an IDENTICAL visual signature (bg=32,32,32|align=center|border=0|avatar=0) — reader can't tell them apart
- 🟡 `deep-chat-right-panel-multi` [tablet] `[data-role="user"] vs [data-role="assistant"]` — two DIFFERENT roles ("user" vs "assistant") render with an IDENTICAL visual signature (bg=32,32,32|align=center|border=0|avatar=0) — reader can't tell them apart
- … +15 more (see JSONL)

### D1 (212)

- 🟡 `chat` [desktop] `span[data-slot=select-value].flex.flex-1` — text truncated (hidden 6px) but parent has 40px free — could show "Select Model"
- 🟡 `chat` [desktop] `span[data-slot=select-value].flex.flex-1` — text truncated (hidden 6px) but parent has 40px free — could show "Select Model"
- 🟡 `chat` [mobile] `span[data-slot=select-value].flex.flex-1` — text truncated (hidden 6px) but parent has 40px free — could show "Select Model"
- 🟡 `chat` [mobile] `span[data-slot=select-value].flex.flex-1` — text truncated (hidden 6px) but parent has 40px free — could show "Select Model"
- 🟡 `chat` [tablet] `span[data-slot=select-value].flex.flex-1` — text truncated (hidden 6px) but parent has 40px free — could show "Select Model"
- 🟡 `chat` [tablet] `span[data-slot=select-value].flex.flex-1` — text truncated (hidden 6px) but parent has 40px free — could show "Select Model"
- 🟡 `deep-chat-attachments` [desktop] `span[data-slot=select-value].flex.flex-1` — text truncated (hidden 6px) but parent has 40px free — could show "Select Model"
- 🟡 `deep-chat-attachments` [mobile] `h5.text-lg.font-semibold` — text truncated (hidden 61px) but parent has 72px free — could show "Message with attachments"
- 🟡 `deep-chat-attachments` [mobile] `span[data-slot=select-value].flex.flex-1` — text truncated (hidden 6px) but parent has 40px free — could show "Select Model"
- 🟡 `deep-chat-attachments` [tablet] `[data-testid="chat-keyboard-tips"]` — text truncated (hidden 6px) but parent has 94px free — could show "Tips: Ctrl+Enter to send"
- 🟡 `deep-chat-attachments` [tablet] `span[data-slot=select-value].flex.flex-1` — text truncated (hidden 6px) but parent has 40px free — could show "Select Model"
- 🟡 `deep-chat-branched` [desktop] `span[data-slot=select-value].flex.flex-1` — text truncated (hidden 6px) but parent has 40px free — could show "Select Model"
- 🟡 `deep-chat-branched` [mobile] `span[data-slot=select-value].flex.flex-1` — text truncated (hidden 6px) but parent has 40px free — could show "Select Model"
- 🟡 `deep-chat-branched` [tablet] `[data-testid="chat-keyboard-tips"]` — text truncated (hidden 6px) but parent has 94px free — could show "Tips: Ctrl+Enter to send"
- 🟡 `deep-chat-branched` [tablet] `span[data-slot=select-value].flex.flex-1` — text truncated (hidden 6px) but parent has 40px free — could show "Select Model"
- 🟡 `deep-chat-elicitation` [desktop] `span[data-slot=select-value].flex.flex-1` — text truncated (hidden 6px) but parent has 40px free — could show "Select Model"
- 🟡 `deep-chat-elicitation` [mobile] `h5.text-lg.font-semibold` — text truncated (hidden 65px) but parent has 72px free — could show "Elicitation — awaiting i"
- 🟡 `deep-chat-elicitation` [mobile] `span[data-slot=select-value].flex.flex-1` — text truncated (hidden 6px) but parent has 40px free — could show "Select Model"
- 🟡 `deep-chat-elicitation` [tablet] `[data-testid="chat-keyboard-tips"]` — text truncated (hidden 6px) but parent has 94px free — could show "Tips: Ctrl+Enter to send"
- 🟡 `deep-chat-elicitation` [tablet] `span[data-slot=select-value].flex.flex-1` — text truncated (hidden 6px) but parent has 40px free — could show "Select Model"
- 🟡 `deep-chat-long` [desktop] `span[data-slot=select-value].flex.flex-1` — text truncated (hidden 6px) but parent has 40px free — could show "Select Model"
- 🟡 `deep-chat-long` [mobile] `span.font-medium.truncate` — text truncated (hidden 45px) but parent has 237px free — could show "report.pdf"
- 🟡 `deep-chat-long` [mobile] `span[data-slot=select-value].flex.flex-1` — text truncated (hidden 6px) but parent has 40px free — could show "Select Model"
- 🟡 `deep-chat-long` [tablet] `[data-testid="chat-keyboard-tips"]` — text truncated (hidden 6px) but parent has 94px free — could show "Tips: Ctrl+Enter to send"
- 🟡 `deep-chat-long` [tablet] `span[data-slot=select-value].flex.flex-1` — text truncated (hidden 6px) but parent has 40px free — could show "Select Model"
- 🟡 `deep-chat-mcp-toolcall-completed` [desktop] `span[data-slot=select-value].flex.flex-1` — text truncated (hidden 6px) but parent has 40px free — could show "Select Model"
- 🟡 `deep-chat-mcp-toolcall-completed` [mobile] `span[data-slot=select-value].flex.flex-1` — text truncated (hidden 6px) but parent has 40px free — could show "Select Model"
- 🟡 `deep-chat-mcp-toolcall-completed` [tablet] `[data-testid="chat-keyboard-tips"]` — text truncated (hidden 6px) but parent has 94px free — could show "Tips: Ctrl+Enter to send"
- 🟡 `deep-chat-mcp-toolcall-completed` [tablet] `span[data-slot=select-value].flex.flex-1` — text truncated (hidden 6px) but parent has 40px free — could show "Select Model"
- 🟡 `deep-chat-mcp-toolcall-error` [desktop] `span[data-slot=select-value].flex.flex-1` — text truncated (hidden 6px) but parent has 40px free — could show "Select Model"
- … +182 more (see JSONL)

### G5 (474)

- 🟡 `auth` [mobile] `button.pointer-events-auto.text-muted-foreground` — tap target 16×16px < 44px (mobile)
- 🟡 `auth` [mobile] `button.pointer-events-auto.text-muted-foreground` — tap target 16×16px < 44px (mobile)
- 🟡 `auth` [mobile] `button.pointer-events-auto.text-muted-foreground` — tap target 16×16px < 44px (mobile)
- 🟡 `auth-link-account` [mobile] `a` — tap target 47×16px < 44px (mobile)
- 🟡 `auth-link-account` [mobile] `button.pointer-events-auto.text-muted-foreground` — tap target 16×16px < 44px (mobile)
- 🟡 `chat` [mobile] `[data-testid="mcp-chip-e0000000-0000-0000-0000-0000000000e1-close"]` — tap target 12×12px < 44px (mobile)
- 🟡 `chats` [mobile] `[data-testid="chat-conversation-select-11111111-1111-1111-1111-111111111111"]` — tap target 16×16px < 44px (mobile)
- 🟡 `deep-chat-attachments` [mobile] `[data-testid="mcp-chip-e0000000-0000-0000-0000-0000000000e1-close"]` — tap target 12×12px < 44px (mobile)
- 🟡 `deep-chat-branched` [mobile] `[data-testid="mcp-chip-e0000000-0000-0000-0000-0000000000e1-close"]` — tap target 12×12px < 44px (mobile)
- 🟡 `deep-chat-elicitation` [mobile] `[data-testid="elicitation-field-include_headers"]` — tap target 32×18px < 44px (mobile)
- 🟡 `deep-chat-elicitation` [mobile] `[data-testid="mcp-chip-e0000000-0000-0000-0000-0000000000e1-close"]` — tap target 12×12px < 44px (mobile)
- 🟡 `deep-chat-long` [mobile] `a.wrap-anywhere.font-medium` — tap target 28×19px < 44px (mobile)
- 🟡 `deep-chat-long` [mobile] `a#user-content-user-content-fnref-1-2.wrap-anywhere.font-medium` — tap target 9×16px < 44px (mobile)
- 🟡 `deep-chat-long` [mobile] `a#user-content-user-content-fnref-1.wrap-anywhere.font-medium` — tap target 9×16px < 44px (mobile)
- 🟡 `deep-chat-long` [mobile] `a#user-content-user-content-fnref-2.wrap-anywhere.font-medium` — tap target 9×16px < 44px (mobile)
- 🟡 `deep-chat-long` [mobile] `input` — tap target 13×13px < 44px (mobile)
- 🟡 `deep-chat-mcp-toolcall-completed` [mobile] `[data-testid="mcp-chip-e0000000-0000-0000-0000-0000000000e1-close"]` — tap target 12×12px < 44px (mobile)
- 🟡 `deep-chat-mcp-toolcall-error` [mobile] `[data-testid="mcp-chip-e0000000-0000-0000-0000-0000000000e1-close"]` — tap target 12×12px < 44px (mobile)
- 🟡 `deep-chat-rendering-showcase` [mobile] `[data-testid="mcp-chip-e0000000-0000-0000-0000-0000000000e1-close"]` — tap target 12×12px < 44px (mobile)
- 🟡 `deep-chat-right-panel-file` [mobile] `[data-testid="mcp-chip-e0000000-0000-0000-0000-0000000000e1-close"]` — tap target 12×12px < 44px (mobile)
- 🟡 `deep-chat-right-panel-file` [mobile] `a.wrap-anywhere.font-medium` — tap target 28×19px < 44px (mobile)
- 🟡 `deep-chat-right-panel-file` [mobile] `a#user-content-user-content-fnref-1-2.wrap-anywhere.font-medium` — tap target 9×16px < 44px (mobile)
- 🟡 `deep-chat-right-panel-file` [mobile] `a#user-content-user-content-fnref-1.wrap-anywhere.font-medium` — tap target 9×16px < 44px (mobile)
- 🟡 `deep-chat-right-panel-file` [mobile] `a#user-content-user-content-fnref-2.wrap-anywhere.font-medium` — tap target 9×16px < 44px (mobile)
- 🟡 `deep-chat-right-panel-file` [mobile] `button.absolute.right-1.5` — tap target 16×16px < 44px (mobile)
- 🟡 `deep-chat-right-panel-file` [mobile] `input` — tap target 13×13px < 44px (mobile)
- 🟡 `deep-chat-right-panel-literature` [mobile] `[data-testid="lit-screening-record-checkbox-doi:10.1000/demo.1"]` — tap target 16×16px < 44px (mobile)
- 🟡 `deep-chat-right-panel-literature` [mobile] `[data-testid="lit-screening-record-checkbox-doi:10.1000/demo.2"]` — tap target 16×16px < 44px (mobile)
- 🟡 `deep-chat-right-panel-literature` [mobile] `[data-testid="lit-screening-select-all-checkbox"]` — tap target 16×16px < 44px (mobile)
- 🟡 `deep-chat-right-panel-literature` [mobile] `a.wrap-anywhere.font-medium` — tap target 28×19px < 44px (mobile)
- … +444 more (see JSONL)

### G7 (143)

- 🟡 `deep-chat-long` [desktop] `a.wrap-anywhere.font-medium` — focus ring (2px) clipped by overflow-x ancestor div.w-full.overflow-x-auto — cut 2px
- 🟡 `deep-chat-long` [desktop] `a.wrap-anywhere.font-medium` — focus ring (2px) clipped by overflow-x ancestor div.w-full.overflow-x-auto — cut 2px
- 🟡 `deep-chat-long` [desktop] `a.wrap-anywhere.font-medium` — focus ring (2px) clipped by overflow-x ancestor div.w-full.overflow-x-auto — cut 2px
- 🟡 `deep-chat-long` [mobile] `a.wrap-anywhere.font-medium` — focus ring (2px) clipped by overflow-x ancestor div.w-full.overflow-x-auto — cut 2px
- 🟡 `deep-chat-long` [mobile] `a.wrap-anywhere.font-medium` — focus ring (2px) clipped by overflow-x ancestor div.w-full.overflow-x-auto — cut 2px
- 🟡 `deep-chat-long` [mobile] `a.wrap-anywhere.font-medium` — focus ring (2px) clipped by overflow-x ancestor div.w-full.overflow-x-auto — cut 2px
- 🟡 `deep-chat-long` [mobile] `a#user-content-user-content-fnref-1.wrap-anywhere.font-medium` — focus ring (2px) clipped by overflow-x ancestor div — cut 368px
- 🟡 `deep-chat-long` [tablet] `a.wrap-anywhere.font-medium` — focus ring (2px) clipped by overflow-x ancestor div.w-full.overflow-x-auto — cut 2px
- 🟡 `deep-chat-long` [tablet] `a.wrap-anywhere.font-medium` — focus ring (2px) clipped by overflow-x ancestor div.w-full.overflow-x-auto — cut 2px
- 🟡 `deep-chat-right-panel-file` [desktop] `a.wrap-anywhere.font-medium` — focus ring (2px) clipped by overflow-x ancestor div.w-full.overflow-x-auto — cut 2px
- 🟡 `deep-chat-right-panel-file` [mobile] `a.wrap-anywhere.font-medium` — focus ring (2px) clipped by overflow-x ancestor div.w-full.overflow-x-auto — cut 2px
- 🟡 `deep-chat-right-panel-file` [mobile] `a.wrap-anywhere.font-medium` — focus ring (2px) clipped by overflow-x ancestor div.w-full.overflow-x-auto — cut 2px
- 🟡 `deep-chat-right-panel-file` [mobile] `a.wrap-anywhere.font-medium` — focus ring (2px) clipped by overflow-x ancestor div.w-full.overflow-x-auto — cut 2px
- 🟡 `deep-chat-right-panel-file` [mobile] `a#user-content-user-content-fnref-1.wrap-anywhere.font-medium` — focus ring (2px) clipped by overflow-x ancestor div — cut 368px
- 🟡 `deep-chat-right-panel-file` [tablet] `a.wrap-anywhere.font-medium` — focus ring (2px) clipped by overflow-x ancestor div.w-full.overflow-x-auto — cut 2px
- 🟡 `deep-chat-right-panel-file` [tablet] `a.wrap-anywhere.font-medium` — focus ring (2px) clipped by overflow-x ancestor div.w-full.overflow-x-auto — cut 2px
- 🟡 `deep-chat-right-panel-file` [tablet] `a.wrap-anywhere.font-medium` — focus ring (2px) clipped by overflow-x ancestor div.w-full.overflow-x-auto — cut 2px
- 🟡 `deep-chat-right-panel-file` [tablet] `a.wrap-anywhere.font-medium` — focus ring (2px) clipped by overflow-x ancestor div.w-full.overflow-x-auto — cut 2px
- 🟡 `deep-chat-right-panel-file` [tablet] `a.wrap-anywhere.font-medium` — focus ring (2px) clipped by overflow-x ancestor div.w-full.overflow-x-auto — cut 2px
- 🟡 `deep-chat-right-panel-file` [tablet] `a#user-content-user-content-fnref-1.wrap-anywhere.font-medium` — focus ring (2px) clipped by overflow-x ancestor div — cut 430px
- 🟡 `deep-chat-right-panel-literature` [desktop] `a.wrap-anywhere.font-medium` — focus ring (2px) clipped by overflow-x ancestor div.w-full.overflow-x-auto — cut 2px
- 🟡 `deep-chat-right-panel-literature` [mobile] `a.wrap-anywhere.font-medium` — focus ring (2px) clipped by overflow-x ancestor div.w-full.overflow-x-auto — cut 2px
- 🟡 `deep-chat-right-panel-literature` [mobile] `a.wrap-anywhere.font-medium` — focus ring (2px) clipped by overflow-x ancestor div.w-full.overflow-x-auto — cut 2px
- 🟡 `deep-chat-right-panel-literature` [mobile] `a.wrap-anywhere.font-medium` — focus ring (2px) clipped by overflow-x ancestor div.w-full.overflow-x-auto — cut 2px
- 🟡 `deep-chat-right-panel-literature` [mobile] `a#user-content-user-content-fnref-1.wrap-anywhere.font-medium` — focus ring (2px) clipped by overflow-x ancestor div — cut 368px
- 🟡 `deep-chat-right-panel-literature` [tablet] `a.wrap-anywhere.font-medium` — focus ring (2px) clipped by overflow-x ancestor div.w-full.overflow-x-auto — cut 2px
- 🟡 `deep-chat-right-panel-literature` [tablet] `a.wrap-anywhere.font-medium` — focus ring (2px) clipped by overflow-x ancestor div.w-full.overflow-x-auto — cut 2px
- 🟡 `deep-chat-right-panel-literature` [tablet] `a.wrap-anywhere.font-medium` — focus ring (2px) clipped by overflow-x ancestor div.w-full.overflow-x-auto — cut 2px
- 🟡 `deep-chat-right-panel-literature` [tablet] `a.wrap-anywhere.font-medium` — focus ring (2px) clipped by overflow-x ancestor div.w-full.overflow-x-auto — cut 2px
- 🟡 `deep-chat-right-panel-literature` [tablet] `a.wrap-anywhere.font-medium` — focus ring (2px) clipped by overflow-x ancestor div.w-full.overflow-x-auto — cut 2px
- … +113 more (see JSONL)

### I1 (137)

- 🟡 `chat` [mobile] `[data-testid="chat-input-add-btn"]` — interactive <button> "" occluded at center by span.[&_svg]:size-4 (hit-test miss)
- 🟡 `deep-chat-attachments` [mobile] `[data-testid="chat-export-btn"]` — interactive <button> "Export" occluded at center by [data-testid="ullm-model-select"] (hit-test miss)
- 🟡 `deep-chat-branched` [mobile] `[data-testid="chat-export-btn"]` — interactive <button> "Export" occluded at center by [data-testid="ullm-model-select"] (hit-test miss)
- 🟡 `deep-chat-elicitation` [mobile] `[data-testid="chat-export-btn"]` — interactive <button> "Export" occluded at center by [data-testid="ullm-model-select"] (hit-test miss)
- 🟡 `deep-chat-long` [mobile] `[data-testid="chat-export-btn"]` — interactive <button> "Export" occluded at center by [data-testid="ullm-model-select"] (hit-test miss)
- 🟡 `deep-chat-mcp-toolcall-completed` [mobile] `[data-testid="chat-export-btn"]` — interactive <button> "Export" occluded at center by [data-testid="ullm-model-select"] (hit-test miss)
- 🟡 `deep-chat-mcp-toolcall-error` [mobile] `[data-testid="chat-export-btn"]` — interactive <button> "Export" occluded at center by [data-testid="ullm-model-select"] (hit-test miss)
- 🟡 `deep-chat-rendering-showcase` [desktop] `button.cursor-pointer.p-1` — interactive <button> "" occluded at center by div.h-full.flex (hit-test miss)
- 🟡 `deep-chat-rendering-showcase` [mobile] `[data-testid="chat-export-btn"]` — interactive <button> "Export" occluded at center by [data-testid="ullm-model-select"] (hit-test miss)
- 🟡 `deep-chat-rendering-showcase` [mobile] `button.cursor-pointer.p-1` — interactive <button> "" occluded at center by div.flex.justify-between (hit-test miss)
- 🟡 `deep-chat-rendering-showcase` [tablet] `button.cursor-pointer.p-1` — interactive <button> "" occluded at center by div.h-full.flex (hit-test miss)
- 🟡 `deep-chat-right-panel-file` [desktop] `[data-testid="chat-message-copy-btn"]` — interactive <button> "" occluded at center by div.flex.gap-1 (hit-test miss)
- 🟡 `deep-chat-right-panel-file` [desktop] `[data-testid="edit-message-button"]` — interactive <button> "" occluded at center by div.flex.gap-1 (hit-test miss)
- 🟡 `deep-chat-right-panel-file` [mobile] `[data-testid="chat-branch-next-btn"]` — interactive <button> "" occluded at center by div.w-full.flex (hit-test miss)
- 🟡 `deep-chat-right-panel-file` [mobile] `[data-testid="chat-branch-prev-btn"]` — interactive <button> "" occluded at center by div.w-full.flex (hit-test miss)
- 🟡 `deep-chat-right-panel-file` [mobile] `[data-testid="chat-export-btn"]` — interactive <button> "Export" occluded at center by div.w-full.flex (hit-test miss)
- 🟡 `deep-chat-right-panel-file` [mobile] `[data-testid="chat-input-add-btn"]` — interactive <button> "" occluded at center by div.w-full.flex (hit-test miss)
- 🟡 `deep-chat-right-panel-file` [mobile] `[data-testid="chat-input-send-btn"]` — interactive <button> "" occluded at center by div.w-full.flex (hit-test miss)
- 🟡 `deep-chat-right-panel-file` [mobile] `[data-testid="chat-message-copy-btn"]` — interactive <button> "" occluded at center by div.w-full.flex (hit-test miss)
- 🟡 `deep-chat-right-panel-file` [mobile] `[data-testid="chat-message-textarea"]` — interactive <textarea> "" occluded at center by div.w-full.flex (hit-test miss)
- 🟡 `deep-chat-right-panel-file` [mobile] `[data-testid="chat-title-edit-btn"]` — interactive <button> "" occluded at center by div.flex.flex-col (hit-test miss)
- 🟡 `deep-chat-right-panel-file` [mobile] `[data-testid="conversation-back-button"]` — interactive <button> "" occluded at center by div.flex.flex-col (hit-test miss)
- 🟡 `deep-chat-right-panel-file` [mobile] `[data-testid="edit-message-button"]` — interactive <button> "" occluded at center by div.w-full.flex (hit-test miss)
- 🟡 `deep-chat-right-panel-file` [mobile] `[data-testid="file-card-remove-btn"]` — interactive <button> "" occluded at center by div.w-full.flex (hit-test miss)
- 🟡 `deep-chat-right-panel-file` [mobile] `[data-testid="mcp-chip-e0000000-0000-0000-0000-0000000000e1-close"]` — interactive <button> "" occluded at center by div.w-full.flex (hit-test miss)
- 🟡 `deep-chat-right-panel-file` [mobile] `[data-testid="memory-status-pill"]` — interactive <span> "Memory: auto" occluded at center by div.w-full.flex (hit-test miss)
- 🟡 `deep-chat-right-panel-file` [mobile] `[data-testid="ullm-model-select"]` — interactive <button> "Select Model▼" occluded at center by div.w-full.flex (hit-test miss)
- 🟡 `deep-chat-right-panel-file` [mobile] `a.wrap-anywhere.font-medium` — interactive <a> "link" occluded at center by div.w-full.flex (hit-test miss)
- 🟡 `deep-chat-right-panel-file` [mobile] `button[data-slot=attachment-trigger].absolute.inset-0` — interactive <button> "" occluded at center by div.w-full.flex (hit-test miss)
- 🟡 `deep-chat-right-panel-file` [tablet] `[data-testid="chat-input-add-btn"]` — interactive <button> "" occluded at center by [data-testid="ullm-model-select"] (hit-test miss)
- … +107 more (see JSONL)

### I4 (3)

- 🟡 `deep-chat-right-panel-file` [mobile] `body` — overlay open but body scroll NOT locked
- 🟡 `deep-chat-right-panel-literature` [mobile] `body` — overlay open but body scroll NOT locked
- 🟡 `deep-chat-right-panel-multi` [mobile] `body` — overlay open but body scroll NOT locked

### J6 (4)

- 🟡 `chats` [mobile] `div.flex.items-center` — peer icon-only action group mixes button variants: {outline, default} — chat-history-search-toggle-btn=outline, chat-history-header-new-chat-btn=default
- 🟡 `chats` [mobile] `div.flex.items-center` — peer icon-only action group mixes button variants: {outline, default} — chat-history-search-toggle-btn=outline, chat-history-header-new-chat-btn=default
- 🟡 `chats` [mobile] `div.flex.items-center` — peer icon-only action group mixes button variants: {outline, default} — chat-history-search-toggle-btn=outline, chat-history-header-new-chat-btn=default
- 🟡 `seeded-chat-history-list` [mobile] `div.flex.items-center` — peer icon-only action group mixes button variants: {outline, default} — chat-history-search-toggle-btn=outline, chat-history-header-new-chat-btn=default

### J7 (65)

- 🟡 `deep-chat-long` [mobile] `[data-testid="file-viewer-copy-btn"]` — "copy" control on the right here but left in the majority of containers (920 left / 13 right) — inconsistent placement
- 🟡 `deep-chat-right-panel-file` [mobile] `[data-testid="file-viewer-copy-btn"]` — "copy" control on the right here but left in the majority of containers (920 left / 13 right) — inconsistent placement
- 🟡 `deep-chat-right-panel-file` [tablet] `[data-testid="file-viewer-copy-btn"]` — "copy" control on the right here but left in the majority of containers (920 left / 13 right) — inconsistent placement
- 🟡 `deep-chat-right-panel-literature` [mobile] `[data-testid="file-viewer-copy-btn"]` — "copy" control on the right here but left in the majority of containers (920 left / 13 right) — inconsistent placement
- 🟡 `deep-chat-right-panel-literature` [tablet] `[data-testid="file-viewer-copy-btn"]` — "copy" control on the right here but left in the majority of containers (920 left / 13 right) — inconsistent placement
- 🟡 `deep-chat-right-panel-multi` [mobile] `[data-testid="file-viewer-copy-btn"]` — "copy" control on the right here but left in the majority of containers (920 left / 13 right) — inconsistent placement
- 🟡 `deep-chat-right-panel-multi` [tablet] `[data-testid="file-viewer-copy-btn"]` — "copy" control on the right here but left in the majority of containers (920 left / 13 right) — inconsistent placement
- 🟡 `deep-chat-streaming` [mobile] `[data-testid="file-viewer-copy-btn"]` — "copy" control on the right here but left in the majority of containers (920 left / 13 right) — inconsistent placement
- 🟡 `overlay-assign-group-drawer` [desktop] `[data-testid="layout-drawer-close-button"]` — "close" control on the left here but right in the majority of containers (52 left / 82 right) — inconsistent placement
- 🟡 `overlay-assign-group-drawer` [mobile] `[data-testid="layout-drawer-close-button"]` — "close" control on the left here but right in the majority of containers (52 left / 82 right) — inconsistent placement
- 🟡 `overlay-assign-group-drawer` [tablet] `[data-testid="layout-drawer-close-button"]` — "close" control on the left here but right in the majority of containers (52 left / 82 right) — inconsistent placement
- 🟡 `overlay-assistant-form-drawer` [desktop] `[data-testid="layout-drawer-close-button"]` — "close" control on the left here but right in the majority of containers (52 left / 82 right) — inconsistent placement
- 🟡 `overlay-assistant-form-drawer` [mobile] `[data-testid="layout-drawer-close-button"]` — "close" control on the left here but right in the majority of containers (52 left / 82 right) — inconsistent placement
- 🟡 `overlay-assistant-form-drawer` [tablet] `[data-testid="layout-drawer-close-button"]` — "close" control on the left here but right in the majority of containers (52 left / 82 right) — inconsistent placement
- 🟡 `overlay-create-user-drawer` [desktop] `[data-testid="layout-drawer-close-button"]` — "close" control on the left here but right in the majority of containers (52 left / 82 right) — inconsistent placement
- 🟡 `overlay-create-user-drawer` [mobile] `[data-testid="layout-drawer-close-button"]` — "close" control on the left here but right in the majority of containers (52 left / 82 right) — inconsistent placement
- 🟡 `overlay-create-user-drawer` [tablet] `[data-testid="layout-drawer-close-button"]` — "close" control on the left here but right in the majority of containers (52 left / 82 right) — inconsistent placement
- 🟡 `overlay-edit-user-drawer` [desktop] `[data-testid="layout-drawer-close-button"]` — "close" control on the left here but right in the majority of containers (52 left / 82 right) — inconsistent placement
- 🟡 `overlay-edit-user-drawer` [mobile] `[data-testid="layout-drawer-close-button"]` — "close" control on the left here but right in the majority of containers (52 left / 82 right) — inconsistent placement
- 🟡 `overlay-edit-user-drawer` [tablet] `[data-testid="layout-drawer-close-button"]` — "close" control on the left here but right in the majority of containers (52 left / 82 right) — inconsistent placement
- 🟡 `overlay-edit-user-group-drawer` [desktop] `[data-testid="layout-drawer-close-button"]` — "close" control on the left here but right in the majority of containers (52 left / 82 right) — inconsistent placement
- 🟡 `overlay-edit-user-group-drawer` [mobile] `[data-testid="layout-drawer-close-button"]` — "close" control on the left here but right in the majority of containers (52 left / 82 right) — inconsistent placement
- 🟡 `overlay-edit-user-group-drawer` [tablet] `[data-testid="layout-drawer-close-button"]` — "close" control on the left here but right in the majority of containers (52 left / 82 right) — inconsistent placement
- 🟡 `overlay-group-llm-providers-assignment` [desktop] `[data-testid="layout-drawer-close-button"]` — "close" control on the left here but right in the majority of containers (52 left / 82 right) — inconsistent placement
- 🟡 `overlay-group-llm-providers-assignment` [mobile] `[data-testid="layout-drawer-close-button"]` — "close" control on the left here but right in the majority of containers (52 left / 82 right) — inconsistent placement
- 🟡 `overlay-group-llm-providers-assignment` [tablet] `[data-testid="layout-drawer-close-button"]` — "close" control on the left here but right in the majority of containers (52 left / 82 right) — inconsistent placement
- 🟡 `overlay-group-mcp-servers-assignment` [desktop] `[data-testid="layout-drawer-close-button"]` — "close" control on the left here but right in the majority of containers (52 left / 82 right) — inconsistent placement
- 🟡 `overlay-group-mcp-servers-assignment` [mobile] `[data-testid="layout-drawer-close-button"]` — "close" control on the left here but right in the majority of containers (52 left / 82 right) — inconsistent placement
- 🟡 `overlay-group-mcp-servers-assignment` [tablet] `[data-testid="layout-drawer-close-button"]` — "close" control on the left here but right in the majority of containers (52 left / 82 right) — inconsistent placement
- 🟡 `overlay-group-members-drawer` [desktop] `[data-testid="layout-drawer-close-button"]` — "close" control on the left here but right in the majority of containers (52 left / 82 right) — inconsistent placement
- … +35 more (see JSONL)

### K1 (14)

- 🟡 `deep-chat-long` [desktop] `[data-testid="project-header-chip-tag"]` — persistent context "project-header-chip-tag" is a DESCENDANT of scroll container div.flex-1.overflow-y-auto — scrolls out of view (should be pinned chrome)
- 🟡 `deep-chat-long` [tablet] `[data-testid="project-header-chip-tag"]` — persistent context "project-header-chip-tag" is a DESCENDANT of scroll container div.flex-1.overflow-y-auto — scrolls out of view (should be pinned chrome)
- 🟡 `deep-chat-rendering-showcase` [desktop] `[data-testid="project-header-chip-tag"]` — persistent context "project-header-chip-tag" is a DESCENDANT of scroll container div.flex-1.overflow-y-auto — scrolls out of view (should be pinned chrome)
- 🟡 `deep-chat-rendering-showcase` [tablet] `[data-testid="project-header-chip-tag"]` — persistent context "project-header-chip-tag" is a DESCENDANT of scroll container div.flex-1.overflow-y-auto — scrolls out of view (should be pinned chrome)
- 🟡 `deep-chat-right-panel-file` [desktop] `[data-testid="project-header-chip-tag"]` — persistent context "project-header-chip-tag" is a DESCENDANT of scroll container div.flex-1.overflow-y-auto — scrolls out of view (should be pinned chrome)
- 🟡 `deep-chat-right-panel-file` [tablet] `[data-testid="project-header-chip-tag"]` — persistent context "project-header-chip-tag" is a DESCENDANT of scroll container div.flex-1.overflow-y-auto — scrolls out of view (should be pinned chrome)
- 🟡 `deep-chat-right-panel-literature` [desktop] `[data-testid="project-header-chip-tag"]` — persistent context "project-header-chip-tag" is a DESCENDANT of scroll container div.flex-1.overflow-y-auto — scrolls out of view (should be pinned chrome)
- 🟡 `deep-chat-right-panel-literature` [tablet] `[data-testid="project-header-chip-tag"]` — persistent context "project-header-chip-tag" is a DESCENDANT of scroll container div.flex-1.overflow-y-auto — scrolls out of view (should be pinned chrome)
- 🟡 `deep-chat-right-panel-multi` [desktop] `[data-testid="project-header-chip-tag"]` — persistent context "project-header-chip-tag" is a DESCENDANT of scroll container div.flex-1.overflow-y-auto — scrolls out of view (should be pinned chrome)
- 🟡 `deep-chat-right-panel-multi` [tablet] `[data-testid="project-header-chip-tag"]` — persistent context "project-header-chip-tag" is a DESCENDANT of scroll container div.flex-1.overflow-y-auto — scrolls out of view (should be pinned chrome)
- 🟡 `deep-chat-streaming` [desktop] `[data-testid="project-header-chip-tag"]` — persistent context "project-header-chip-tag" is a DESCENDANT of scroll container div.flex-1.overflow-y-auto — scrolls out of view (should be pinned chrome)
- 🟡 `deep-chat-streaming` [tablet] `[data-testid="project-header-chip-tag"]` — persistent context "project-header-chip-tag" is a DESCENDANT of scroll container div.flex-1.overflow-y-auto — scrolls out of view (should be pinned chrome)
- 🟡 `seeded-s5-conversation-error` [desktop] `[data-testid="project-header-chip-tag"]` — persistent context "project-header-chip-tag" is a DESCENDANT of scroll container div.flex-1.overflow-y-auto — scrolls out of view (should be pinned chrome)
- 🟡 `seeded-s5-conversation-error` [tablet] `[data-testid="project-header-chip-tag"]` — persistent context "project-header-chip-tag" is a DESCENDANT of scroll container div.flex-1.overflow-y-auto — scrolls out of view (should be pinned chrome)

### L2 (21)

- 🟡 `deep-chat-long` [desktop] `pre.language-mermaid.bg-[var(--sdm-bg,inherit]` — mermaid source ("flowchart TD    A[User m") did not render to <svg>
- 🟡 `deep-chat-long` [mobile] `pre.language-mermaid.bg-[var(--sdm-bg,inherit]` — mermaid source ("flowchart TD    A[User m") did not render to <svg>
- 🟡 `deep-chat-long` [tablet] `pre.language-mermaid.bg-[var(--sdm-bg,inherit]` — mermaid source ("flowchart TD    A[User m") did not render to <svg>
- 🟡 `deep-chat-rendering-showcase` [desktop] `pre.language-mermaid.bg-[var(--sdm-bg,inherit]` — mermaid source ("graph TD  A[Start] --> B") did not render to <svg>
- 🟡 `deep-chat-rendering-showcase` [mobile] `pre.language-mermaid.bg-[var(--sdm-bg,inherit]` — mermaid source ("graph TD  A[Start] --> B") did not render to <svg>
- 🟡 `deep-chat-rendering-showcase` [tablet] `pre.language-mermaid.bg-[var(--sdm-bg,inherit]` — mermaid source ("graph TD  A[Start] --> B") did not render to <svg>
- 🟡 `deep-chat-right-panel-file` [desktop] `pre.language-mermaid.bg-[var(--sdm-bg,inherit]` — mermaid source ("flowchart TD    A[User m") did not render to <svg>
- 🟡 `deep-chat-right-panel-file` [mobile] `pre.language-mermaid.bg-[var(--sdm-bg,inherit]` — mermaid source ("flowchart TD    A[User m") did not render to <svg>
- 🟡 `deep-chat-right-panel-file` [tablet] `pre.language-mermaid.bg-[var(--sdm-bg,inherit]` — mermaid source ("flowchart TD    A[User m") did not render to <svg>
- 🟡 `deep-chat-right-panel-literature` [desktop] `pre.language-mermaid.bg-[var(--sdm-bg,inherit]` — mermaid source ("flowchart TD    A[User m") did not render to <svg>
- 🟡 `deep-chat-right-panel-literature` [mobile] `pre.language-mermaid.bg-[var(--sdm-bg,inherit]` — mermaid source ("flowchart TD    A[User m") did not render to <svg>
- 🟡 `deep-chat-right-panel-literature` [tablet] `pre.language-mermaid.bg-[var(--sdm-bg,inherit]` — mermaid source ("flowchart TD    A[User m") did not render to <svg>
- 🟡 `deep-chat-right-panel-multi` [desktop] `pre.language-mermaid.bg-[var(--sdm-bg,inherit]` — mermaid source ("flowchart TD    A[User m") did not render to <svg>
- 🟡 `deep-chat-right-panel-multi` [mobile] `pre.language-mermaid.bg-[var(--sdm-bg,inherit]` — mermaid source ("flowchart TD    A[User m") did not render to <svg>
- 🟡 `deep-chat-right-panel-multi` [tablet] `pre.language-mermaid.bg-[var(--sdm-bg,inherit]` — mermaid source ("flowchart TD    A[User m") did not render to <svg>
- 🟡 `deep-chat-streaming` [desktop] `pre.language-mermaid.bg-[var(--sdm-bg,inherit]` — mermaid source ("flowchart TD    A[User m") did not render to <svg>
- 🟡 `deep-chat-streaming` [mobile] `pre.language-mermaid.bg-[var(--sdm-bg,inherit]` — mermaid source ("flowchart TD    A[User m") did not render to <svg>
- 🟡 `deep-chat-streaming` [tablet] `pre.language-mermaid.bg-[var(--sdm-bg,inherit]` — mermaid source ("flowchart TD    A[User m") did not render to <svg>
- 🟡 `seeded-s5-conversation-error` [desktop] `pre.language-mermaid.bg-[var(--sdm-bg,inherit]` — mermaid source ("flowchart TD    A[User m") did not render to <svg>
- 🟡 `seeded-s5-conversation-error` [mobile] `pre.language-mermaid.bg-[var(--sdm-bg,inherit]` — mermaid source ("flowchart TD    A[User m") did not render to <svg>
- 🟡 `seeded-s5-conversation-error` [tablet] `pre.language-mermaid.bg-[var(--sdm-bg,inherit]` — mermaid source ("flowchart TD    A[User m") did not render to <svg>

### L3 (21)

- 🟡 `deep-chat-long` [desktop] `pre.language-rust.bg-[var(--sdm-bg,inherit]` — language-tagged code block has 8 token spans / 1 colors — highlighting not applied (single-color plaintext) [dev-serve]
- 🟡 `deep-chat-long` [mobile] `pre.language-rust.bg-[var(--sdm-bg,inherit]` — language-tagged code block has 8 token spans / 1 colors — highlighting not applied (single-color plaintext) [dev-serve]
- 🟡 `deep-chat-long` [tablet] `pre.language-rust.bg-[var(--sdm-bg,inherit]` — language-tagged code block has 8 token spans / 1 colors — highlighting not applied (single-color plaintext) [dev-serve]
- 🟡 `deep-chat-rendering-showcase` [desktop] `pre.language-mermaid.bg-[var(--sdm-bg,inherit]` — language-tagged code block has 8 token spans / 1 colors — highlighting not applied (single-color plaintext) [dev-serve]
- 🟡 `deep-chat-rendering-showcase` [mobile] `pre.language-mermaid.bg-[var(--sdm-bg,inherit]` — language-tagged code block has 8 token spans / 1 colors — highlighting not applied (single-color plaintext) [dev-serve]
- 🟡 `deep-chat-rendering-showcase` [tablet] `pre.language-mermaid.bg-[var(--sdm-bg,inherit]` — language-tagged code block has 8 token spans / 1 colors — highlighting not applied (single-color plaintext) [dev-serve]
- 🟡 `deep-chat-right-panel-file` [desktop] `pre.language-rust.bg-[var(--sdm-bg,inherit]` — language-tagged code block has 8 token spans / 1 colors — highlighting not applied (single-color plaintext) [dev-serve]
- 🟡 `deep-chat-right-panel-file` [mobile] `pre.language-rust.bg-[var(--sdm-bg,inherit]` — language-tagged code block has 8 token spans / 1 colors — highlighting not applied (single-color plaintext) [dev-serve]
- 🟡 `deep-chat-right-panel-file` [tablet] `pre.language-rust.bg-[var(--sdm-bg,inherit]` — language-tagged code block has 8 token spans / 1 colors — highlighting not applied (single-color plaintext) [dev-serve]
- 🟡 `deep-chat-right-panel-literature` [desktop] `pre.language-rust.bg-[var(--sdm-bg,inherit]` — language-tagged code block has 8 token spans / 1 colors — highlighting not applied (single-color plaintext) [dev-serve]
- 🟡 `deep-chat-right-panel-literature` [mobile] `pre.language-rust.bg-[var(--sdm-bg,inherit]` — language-tagged code block has 8 token spans / 1 colors — highlighting not applied (single-color plaintext) [dev-serve]
- 🟡 `deep-chat-right-panel-literature` [tablet] `pre.language-rust.bg-[var(--sdm-bg,inherit]` — language-tagged code block has 8 token spans / 1 colors — highlighting not applied (single-color plaintext) [dev-serve]
- 🟡 `deep-chat-right-panel-multi` [desktop] `pre.language-rust.bg-[var(--sdm-bg,inherit]` — language-tagged code block has 8 token spans / 1 colors — highlighting not applied (single-color plaintext) [dev-serve]
- 🟡 `deep-chat-right-panel-multi` [mobile] `pre.language-rust.bg-[var(--sdm-bg,inherit]` — language-tagged code block has 8 token spans / 1 colors — highlighting not applied (single-color plaintext) [dev-serve]
- 🟡 `deep-chat-right-panel-multi` [tablet] `pre.language-rust.bg-[var(--sdm-bg,inherit]` — language-tagged code block has 8 token spans / 1 colors — highlighting not applied (single-color plaintext) [dev-serve]
- 🟡 `deep-chat-streaming` [desktop] `pre.language-rust.bg-[var(--sdm-bg,inherit]` — language-tagged code block has 8 token spans / 1 colors — highlighting not applied (single-color plaintext) [dev-serve]
- 🟡 `deep-chat-streaming` [mobile] `pre.language-rust.bg-[var(--sdm-bg,inherit]` — language-tagged code block has 8 token spans / 1 colors — highlighting not applied (single-color plaintext) [dev-serve]
- 🟡 `deep-chat-streaming` [tablet] `pre.language-rust.bg-[var(--sdm-bg,inherit]` — language-tagged code block has 8 token spans / 1 colors — highlighting not applied (single-color plaintext) [dev-serve]
- 🟡 `seeded-s5-conversation-error` [desktop] `pre.language-rust.bg-[var(--sdm-bg,inherit]` — language-tagged code block has 8 token spans / 1 colors — highlighting not applied (single-color plaintext) [dev-serve]
- 🟡 `seeded-s5-conversation-error` [mobile] `pre.language-rust.bg-[var(--sdm-bg,inherit]` — language-tagged code block has 8 token spans / 1 colors — highlighting not applied (single-color plaintext) [dev-serve]
- 🟡 `seeded-s5-conversation-error` [tablet] `pre.language-rust.bg-[var(--sdm-bg,inherit]` — language-tagged code block has 8 token spans / 1 colors — highlighting not applied (single-color plaintext) [dev-serve]
