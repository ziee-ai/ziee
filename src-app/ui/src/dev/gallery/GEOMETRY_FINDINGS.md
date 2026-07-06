# Geometry findings (GENERATED)

> `node scripts/gallery-geometry-audit.mjs` (Layer 1 — see `docs/DEFECT_TAXONOMY.md`). Deterministic DOM-geometry rules over every gallery surface × state × viewport. Each row cites surface, viewport, taxonomy class, selector, and measured numbers.

## Totals

| Severity | Count |
|---|---|
| 🔴 HIGH (gating) | 0 |
| 🔵 HIGH (allow-listed) | 3 |
| 🟡 MEDIUM | 874 |
| ⚪ LOW | 1115 |
| **Total** | **1992** |

## By taxonomy class

| Class | HIGH | MEDIUM | LOW | allow-listed |
|---|---|---|---|---|
| A1 | 3 | 0 | 0 | 3 |
| A2 | 0 | 3 | 0 | 0 |
| A3 | 0 | 21 | 0 | 0 |
| A4 | 0 | 0 | 3 | 0 |
| A7 | 0 | 0 | 232 | 0 |
| A9 | 0 | 9 | 0 | 0 |
| B1 | 0 | 16 | 0 | 0 |
| B2 | 0 | 3 | 0 | 0 |
| B8 | 0 | 0 | 18 | 0 |
| C10 | 0 | 6 | 0 | 0 |
| C12 | 0 | 15 | 6 | 0 |
| C7 | 0 | 15 | 0 | 0 |
| D1 | 0 | 162 | 0 | 0 |
| G5 | 0 | 421 | 835 | 0 |
| G7 | 0 | 83 | 0 | 0 |
| I1 | 0 | 80 | 0 | 0 |
| J6 | 0 | 3 | 0 | 0 |
| J7 | 0 | 37 | 0 | 0 |
| NAV | 0 | 0 | 21 | 0 |

## Gating HIGH findings (0)

_None — geometry is clean of un-allow-listed HIGH findings._

## MEDIUM findings (874)

### A2 (3)

- 🟡 `seeded-file-rag-error` [tablet] `code ∩ code` — sibling boxes overlap by 2958px² (100% of smaller)
- 🟡 `settings-file-rag-admin` [tablet] `code ∩ code` — sibling boxes overlap by 2958px² (100% of smaller)
- 🟡 `settings-file-rag-admin` [tablet] `code ∩ code` — sibling boxes overlap by 2958px² (100% of smaller)

### A3 (21)

- 🟡 `chat` [desktop] `span.[&_svg]:size-4` — protrudes 3px past parent [data-testid="ullm-model-retry"] (no overflow clip)
- 🟡 `chat` [mobile] `span.[&_svg]:size-4` — protrudes 3px past parent [data-testid="ullm-model-retry"] (no overflow clip)
- 🟡 `chat` [mobile] `span#base-ui-_r_a_[data-slot=tooltip-trigger].inline-flex.shrink-0` — protrudes 28px past parent div.flex.items-center (no overflow clip)
- 🟡 `chat` [tablet] `span.[&_svg]:size-4` — protrudes 3px past parent [data-testid="ullm-model-retry"] (no overflow clip)
- 🟡 `deep-chat-attachments` [mobile] `[data-testid="chat-export-btn"]` — protrudes 60px past parent div.flex.items-center (no overflow clip)
- 🟡 `deep-chat-mcp-toolcall-completed` [mobile] `[data-testid="chat-export-btn"]` — protrudes 60px past parent div.flex.items-center (no overflow clip)
- 🟡 `deep-chat-mcp-toolcall-completed` [mobile] `[data-testid="mcp-toolcall-details-btn-toolu_running_1"]` — protrudes 71px past parent div.flex.items-center (no overflow clip)
- 🟡 `deep-chat-mcp-toolcall-completed` [mobile] `div.flex.items-center` — protrudes 39px past parent div.flex.items-center (no overflow clip)
- 🟡 `deep-chat-mcp-toolcall-error` [mobile] `[data-testid="chat-export-btn"]` — protrudes 60px past parent div.flex.items-center (no overflow clip)
- 🟡 `deep-chat-mcp-toolcall-error` [mobile] `[data-testid="mcp-toolcall-details-btn-toolu_failed_1"]` — protrudes 71px past parent div.flex.items-center (no overflow clip)
- 🟡 `deep-chat-mcp-toolcall-error` [mobile] `div.flex.items-center` — protrudes 39px past parent div.flex.items-center (no overflow clip)
- 🟡 `deep-chat-tool-failed` [mobile] `[data-testid="chat-export-btn"]` — protrudes 60px past parent div.flex.items-center (no overflow clip)
- 🟡 `deep-chat-tool-running` [mobile] `[data-testid="chat-export-btn"]` — protrudes 60px past parent div.flex.items-center (no overflow clip)
- 🟡 `hardware-monitor` [mobile] `div.flex-1.min-w-80` — protrudes 38px past parent div.flex.gap-3 (no overflow clip)
- 🟡 `hardware-monitor` [mobile] `div.flex-1.min-w-80` — protrudes 38px past parent div.flex.gap-3 (no overflow clip)
- 🟡 `hardware-monitor` [mobile] `div.flex-1.min-w-80` — protrudes 38px past parent div.flex.gap-3 (no overflow clip)
- 🟡 `seeded-hardware-no-gpu` [mobile] `div.flex-1.min-w-80` — protrudes 38px past parent div.flex.gap-3 (no overflow clip)
- 🟡 `seeded-hardware-no-gpu` [mobile] `div.flex-1.min-w-80` — protrudes 38px past parent div.flex.gap-3 (no overflow clip)
- 🟡 `seeded-hardware-no-gpu` [mobile] `div.flex-1.min-w-80` — protrudes 38px past parent div.flex.gap-3 (no overflow clip)
- 🟡 `seeded-s2-project-files-inline-empty` [mobile] `[data-testid="file-project-inline-manage-link"]` — protrudes 42px past parent div (no overflow clip)
- 🟡 `settings-citations` [mobile] `a` — protrudes 4px past parent p.text-sm.leading-relaxed (no overflow clip)

### A9 (9)

- 🟡 `seeded-s4-project-bib-manage-empty` [desktop] `[data-testid="cite-card-44444444-4444-4444-8444-444444444444"]` — peer metric mismatch (element-height): 108px vs group mode 133px among 4 same-kind siblings
- 🟡 `seeded-s4-project-bib-manage-empty` [tablet] `[data-testid="cite-card-22222222-2222-4222-8222-222222222222"]` — peer metric mismatch (element-height): 153px vs group mode 133px among 4 same-kind siblings
- 🟡 `seeded-s4-project-bib-manage-empty` [tablet] `[data-testid="cite-card-44444444-4444-4444-8444-444444444444"]` — peer metric mismatch (element-height): 108px vs group mode 133px among 4 same-kind siblings
- 🟡 `settings-citations` [desktop] `[data-testid="cite-card-44444444-4444-4444-8444-444444444444"]` — peer metric mismatch (element-height): 108px vs group mode 133px among 4 same-kind siblings
- 🟡 `settings-citations` [tablet] `[data-testid="cite-card-22222222-2222-4222-8222-222222222222"]` — peer metric mismatch (element-height): 153px vs group mode 133px among 4 same-kind siblings
- 🟡 `settings-citations` [tablet] `[data-testid="cite-card-44444444-4444-4444-8444-444444444444"]` — peer metric mismatch (element-height): 108px vs group mode 133px among 4 same-kind siblings
- 🟡 `settings-literature-keys` [mobile] `div` — peer metric mismatch (element-height): 356px vs group mode 335px among 4 same-kind siblings
- 🟡 `settings-literature-keys` [tablet] `div` — peer metric mismatch (element-height): 242px vs group mode 221px among 4 same-kind siblings
- 🟡 `settings-literature-keys` [tablet] `div` — peer metric mismatch (element-height): 242px vs group mode 221px among 4 same-kind siblings

### B1 (16)

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

### C10 (6)

- 🟡 `chats` [desktop] `svg` — icon height 64px is 2.00× the adjacent text line-height 32px (oversized)
- 🟡 `chats` [mobile] `svg` — icon height 64px is 2.00× the adjacent text line-height 32px (oversized)
- 🟡 `chats` [tablet] `svg` — icon height 64px is 2.00× the adjacent text line-height 32px (oversized)
- 🟡 `projects` [desktop] `svg` — icon height 64px is 2.00× the adjacent text line-height 32px (oversized)
- 🟡 `projects` [mobile] `svg` — icon height 64px is 2.00× the adjacent text line-height 32px (oversized)
- 🟡 `projects` [tablet] `svg` — icon height 64px is 2.00× the adjacent text line-height 32px (oversized)

### C12 (15)

- 🟡 `deep-chat-attachments` [desktop] `span[data-slot=avatar-fallback].flex.size-full` — bare placeholder circle: rounded-full 32×32px with no img/svg/initials content
- 🟡 `deep-chat-attachments` [mobile] `span[data-slot=avatar-fallback].flex.size-full` — bare placeholder circle: rounded-full 32×32px with no img/svg/initials content
- 🟡 `deep-chat-attachments` [tablet] `span[data-slot=avatar-fallback].flex.size-full` — bare placeholder circle: rounded-full 32×32px with no img/svg/initials content
- 🟡 `deep-chat-mcp-toolcall-completed` [desktop] `span[data-slot=avatar-fallback].flex.size-full` — bare placeholder circle: rounded-full 32×32px with no img/svg/initials content
- 🟡 `deep-chat-mcp-toolcall-completed` [mobile] `span[data-slot=avatar-fallback].flex.size-full` — bare placeholder circle: rounded-full 32×32px with no img/svg/initials content
- 🟡 `deep-chat-mcp-toolcall-completed` [tablet] `span[data-slot=avatar-fallback].flex.size-full` — bare placeholder circle: rounded-full 32×32px with no img/svg/initials content
- 🟡 `deep-chat-mcp-toolcall-error` [desktop] `span[data-slot=avatar-fallback].flex.size-full` — bare placeholder circle: rounded-full 32×32px with no img/svg/initials content
- 🟡 `deep-chat-mcp-toolcall-error` [mobile] `span[data-slot=avatar-fallback].flex.size-full` — bare placeholder circle: rounded-full 32×32px with no img/svg/initials content
- 🟡 `deep-chat-mcp-toolcall-error` [tablet] `span[data-slot=avatar-fallback].flex.size-full` — bare placeholder circle: rounded-full 32×32px with no img/svg/initials content
- 🟡 `deep-chat-tool-failed` [desktop] `span[data-slot=avatar-fallback].flex.size-full` — bare placeholder circle: rounded-full 32×32px with no img/svg/initials content
- 🟡 `deep-chat-tool-failed` [mobile] `span[data-slot=avatar-fallback].flex.size-full` — bare placeholder circle: rounded-full 32×32px with no img/svg/initials content
- 🟡 `deep-chat-tool-failed` [tablet] `span[data-slot=avatar-fallback].flex.size-full` — bare placeholder circle: rounded-full 32×32px with no img/svg/initials content
- 🟡 `deep-chat-tool-running` [desktop] `span[data-slot=avatar-fallback].flex.size-full` — bare placeholder circle: rounded-full 32×32px with no img/svg/initials content
- 🟡 `deep-chat-tool-running` [mobile] `span[data-slot=avatar-fallback].flex.size-full` — bare placeholder circle: rounded-full 32×32px with no img/svg/initials content
- 🟡 `deep-chat-tool-running` [tablet] `span[data-slot=avatar-fallback].flex.size-full` — bare placeholder circle: rounded-full 32×32px with no img/svg/initials content

### C7 (15)

- 🟡 `deep-chat-attachments` [desktop] `[data-role="user"] vs [data-role="assistant"]` — two DIFFERENT roles ("user" vs "assistant") render with an IDENTICAL visual signature (bg=32,32,32|align=center|border=0|avatar=0) — reader can't tell them apart
- 🟡 `deep-chat-attachments` [mobile] `[data-role="user"] vs [data-role="assistant"]` — two DIFFERENT roles ("user" vs "assistant") render with an IDENTICAL visual signature (bg=32,32,32|align=center|border=0|avatar=0) — reader can't tell them apart
- 🟡 `deep-chat-attachments` [tablet] `[data-role="user"] vs [data-role="assistant"]` — two DIFFERENT roles ("user" vs "assistant") render with an IDENTICAL visual signature (bg=32,32,32|align=center|border=0|avatar=0) — reader can't tell them apart
- 🟡 `deep-chat-mcp-toolcall-completed` [desktop] `[data-role="user"] vs [data-role="assistant"]` — two DIFFERENT roles ("user" vs "assistant") render with an IDENTICAL visual signature (bg=32,32,32|align=center|border=0|avatar=0) — reader can't tell them apart
- 🟡 `deep-chat-mcp-toolcall-completed` [mobile] `[data-role="user"] vs [data-role="assistant"]` — two DIFFERENT roles ("user" vs "assistant") render with an IDENTICAL visual signature (bg=32,32,32|align=center|border=0|avatar=0) — reader can't tell them apart
- 🟡 `deep-chat-mcp-toolcall-completed` [tablet] `[data-role="user"] vs [data-role="assistant"]` — two DIFFERENT roles ("user" vs "assistant") render with an IDENTICAL visual signature (bg=32,32,32|align=center|border=0|avatar=0) — reader can't tell them apart
- 🟡 `deep-chat-mcp-toolcall-error` [desktop] `[data-role="user"] vs [data-role="assistant"]` — two DIFFERENT roles ("user" vs "assistant") render with an IDENTICAL visual signature (bg=32,32,32|align=center|border=0|avatar=0) — reader can't tell them apart
- 🟡 `deep-chat-mcp-toolcall-error` [mobile] `[data-role="user"] vs [data-role="assistant"]` — two DIFFERENT roles ("user" vs "assistant") render with an IDENTICAL visual signature (bg=32,32,32|align=center|border=0|avatar=0) — reader can't tell them apart
- 🟡 `deep-chat-mcp-toolcall-error` [tablet] `[data-role="user"] vs [data-role="assistant"]` — two DIFFERENT roles ("user" vs "assistant") render with an IDENTICAL visual signature (bg=32,32,32|align=center|border=0|avatar=0) — reader can't tell them apart
- 🟡 `deep-chat-tool-failed` [desktop] `[data-role="user"] vs [data-role="assistant"]` — two DIFFERENT roles ("user" vs "assistant") render with an IDENTICAL visual signature (bg=32,32,32|align=center|border=0|avatar=0) — reader can't tell them apart
- 🟡 `deep-chat-tool-failed` [mobile] `[data-role="user"] vs [data-role="assistant"]` — two DIFFERENT roles ("user" vs "assistant") render with an IDENTICAL visual signature (bg=32,32,32|align=center|border=0|avatar=0) — reader can't tell them apart
- 🟡 `deep-chat-tool-failed` [tablet] `[data-role="user"] vs [data-role="assistant"]` — two DIFFERENT roles ("user" vs "assistant") render with an IDENTICAL visual signature (bg=32,32,32|align=center|border=0|avatar=0) — reader can't tell them apart
- 🟡 `deep-chat-tool-running` [desktop] `[data-role="user"] vs [data-role="assistant"]` — two DIFFERENT roles ("user" vs "assistant") render with an IDENTICAL visual signature (bg=32,32,32|align=center|border=0|avatar=0) — reader can't tell them apart
- 🟡 `deep-chat-tool-running` [mobile] `[data-role="user"] vs [data-role="assistant"]` — two DIFFERENT roles ("user" vs "assistant") render with an IDENTICAL visual signature (bg=32,32,32|align=center|border=0|avatar=0) — reader can't tell them apart
- 🟡 `deep-chat-tool-running` [tablet] `[data-role="user"] vs [data-role="assistant"]` — two DIFFERENT roles ("user" vs "assistant") render with an IDENTICAL visual signature (bg=32,32,32|align=center|border=0|avatar=0) — reader can't tell them apart

### D1 (162)

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
- 🟡 `deep-chat-mcp-toolcall-completed` [desktop] `span[data-slot=select-value].flex.flex-1` — text truncated (hidden 6px) but parent has 40px free — could show "Select Model"
- 🟡 `deep-chat-mcp-toolcall-completed` [mobile] `span[data-slot=select-value].flex.flex-1` — text truncated (hidden 6px) but parent has 40px free — could show "Select Model"
- 🟡 `deep-chat-mcp-toolcall-completed` [tablet] `[data-testid="chat-keyboard-tips"]` — text truncated (hidden 6px) but parent has 94px free — could show "Tips: Ctrl+Enter to send"
- 🟡 `deep-chat-mcp-toolcall-completed` [tablet] `span[data-slot=select-value].flex.flex-1` — text truncated (hidden 6px) but parent has 40px free — could show "Select Model"
- 🟡 `deep-chat-mcp-toolcall-error` [desktop] `span[data-slot=select-value].flex.flex-1` — text truncated (hidden 6px) but parent has 40px free — could show "Select Model"
- 🟡 `deep-chat-mcp-toolcall-error` [mobile] `span[data-slot=select-value].flex.flex-1` — text truncated (hidden 6px) but parent has 40px free — could show "Select Model"
- 🟡 `deep-chat-mcp-toolcall-error` [tablet] `[data-testid="chat-keyboard-tips"]` — text truncated (hidden 6px) but parent has 94px free — could show "Tips: Ctrl+Enter to send"
- 🟡 `deep-chat-mcp-toolcall-error` [tablet] `span[data-slot=select-value].flex.flex-1` — text truncated (hidden 6px) but parent has 40px free — could show "Select Model"
- 🟡 `deep-chat-tool-failed` [desktop] `span[data-slot=select-value].flex.flex-1` — text truncated (hidden 6px) but parent has 40px free — could show "Select Model"
- 🟡 `deep-chat-tool-failed` [mobile] `span[data-slot=select-value].flex.flex-1` — text truncated (hidden 6px) but parent has 40px free — could show "Select Model"
- 🟡 `deep-chat-tool-failed` [tablet] `[data-testid="chat-keyboard-tips"]` — text truncated (hidden 6px) but parent has 94px free — could show "Tips: Ctrl+Enter to send"
- 🟡 `deep-chat-tool-failed` [tablet] `span[data-slot=select-value].flex.flex-1` — text truncated (hidden 6px) but parent has 40px free — could show "Select Model"
- 🟡 `deep-chat-tool-running` [desktop] `span[data-slot=select-value].flex.flex-1` — text truncated (hidden 6px) but parent has 40px free — could show "Select Model"
- 🟡 `deep-chat-tool-running` [mobile] `span[data-slot=select-value].flex.flex-1` — text truncated (hidden 6px) but parent has 40px free — could show "Select Model"
- 🟡 `deep-chat-tool-running` [tablet] `[data-testid="chat-keyboard-tips"]` — text truncated (hidden 6px) but parent has 94px free — could show "Tips: Ctrl+Enter to send"
- 🟡 `deep-chat-tool-running` [tablet] `span[data-slot=select-value].flex.flex-1` — text truncated (hidden 6px) but parent has 40px free — could show "Select Model"
- 🟡 `hardware-monitor` [desktop] `[data-testid="hardware-monitor-heading"]` — text truncated (hidden 142px) but parent has 895px free — could show "Hardware Monitor"
- 🟡 `hardware-monitor` [desktop] `span` — text truncated (hidden 7px) but parent has 345px free — could show "x"
- 🟡 `hardware-monitor` [desktop] `span` — text truncated (hidden 7px) but parent has 345px free — could show "x"
- … +132 more (see JSONL)

### G5 (421)

- 🟡 `auth` [mobile] `button.pointer-events-auto.text-muted-foreground` — tap target 16×16px < 44px (mobile)
- 🟡 `auth` [mobile] `button.pointer-events-auto.text-muted-foreground` — tap target 16×16px < 44px (mobile)
- 🟡 `auth` [mobile] `button.pointer-events-auto.text-muted-foreground` — tap target 16×16px < 44px (mobile)
- 🟡 `auth-link-account` [mobile] `a` — tap target 47×16px < 44px (mobile)
- 🟡 `auth-link-account` [mobile] `button.pointer-events-auto.text-muted-foreground` — tap target 16×16px < 44px (mobile)
- 🟡 `chat` [mobile] `[data-testid="mcp-chip-e0000000-0000-0000-0000-0000000000e1-close"]` — tap target 12×12px < 44px (mobile)
- 🟡 `chats` [mobile] `[data-testid="chat-conversation-select-11111111-1111-1111-1111-111111111111"]` — tap target 16×16px < 44px (mobile)
- 🟡 `deep-chat-attachments` [mobile] `[data-testid="mcp-chip-e0000000-0000-0000-0000-0000000000e1-close"]` — tap target 12×12px < 44px (mobile)
- 🟡 `deep-chat-mcp-toolcall-completed` [mobile] `[data-testid="mcp-chip-e0000000-0000-0000-0000-0000000000e1-close"]` — tap target 12×12px < 44px (mobile)
- 🟡 `deep-chat-mcp-toolcall-error` [mobile] `[data-testid="mcp-chip-e0000000-0000-0000-0000-0000000000e1-close"]` — tap target 12×12px < 44px (mobile)
- 🟡 `deep-chat-tool-failed` [mobile] `[data-testid="mcp-chip-e0000000-0000-0000-0000-0000000000e1-close"]` — tap target 12×12px < 44px (mobile)
- 🟡 `deep-chat-tool-running` [mobile] `[data-testid="mcp-chip-e0000000-0000-0000-0000-0000000000e1-close"]` — tap target 12×12px < 44px (mobile)
- 🟡 `overlay-assign-group-drawer` [mobile] `[data-testid="user-assign-group-checkboxes-opt-86283a8b-366a-47d7-8d63-d5054ed45fb3"]` — tap target 16×16px < 44px (mobile)
- 🟡 `overlay-assign-group-drawer` [mobile] `[data-testid="user-assign-group-checkboxes-opt-9b96a9cc-a240-4966-834c-bb3aa41464ef"]` — tap target 16×16px < 44px (mobile)
- 🟡 `overlay-assistant-form-drawer` [mobile] `[data-testid="assistant-form-default"]` — tap target 32×18px < 44px (mobile)
- 🟡 `overlay-assistant-form-drawer` [mobile] `[data-testid="assistant-form-enabled"]` — tap target 32×18px < 44px (mobile)
- 🟡 `overlay-create-user-drawer` [mobile] `[data-testid="user-permissions-advanced-switch"]` — tap target 24×14px < 44px (mobile)
- 🟡 `overlay-create-user-drawer` [mobile] `[data-testid="user-permissions-tree-check-assistant_templates::create"]` — tap target 16×16px < 44px (mobile)
- 🟡 `overlay-create-user-drawer` [mobile] `[data-testid="user-permissions-tree-check-assistant_templates::delete"]` — tap target 16×16px < 44px (mobile)
- 🟡 `overlay-create-user-drawer` [mobile] `[data-testid="user-permissions-tree-check-assistant_templates::edit"]` — tap target 16×16px < 44px (mobile)
- 🟡 `overlay-create-user-drawer` [mobile] `[data-testid="user-permissions-tree-check-assistant_templates::read"]` — tap target 16×16px < 44px (mobile)
- 🟡 `overlay-create-user-drawer` [mobile] `[data-testid="user-permissions-tree-check-assistants::create"]` — tap target 16×16px < 44px (mobile)
- 🟡 `overlay-create-user-drawer` [mobile] `[data-testid="user-permissions-tree-check-assistants::delete"]` — tap target 16×16px < 44px (mobile)
- 🟡 `overlay-create-user-drawer` [mobile] `[data-testid="user-permissions-tree-check-assistants::edit"]` — tap target 16×16px < 44px (mobile)
- 🟡 `overlay-create-user-drawer` [mobile] `[data-testid="user-permissions-tree-check-assistants::read"]` — tap target 16×16px < 44px (mobile)
- 🟡 `overlay-create-user-drawer` [mobile] `[data-testid="user-permissions-tree-check-auth_providers::manage"]` — tap target 16×16px < 44px (mobile)
- 🟡 `overlay-create-user-drawer` [mobile] `[data-testid="user-permissions-tree-check-auth_providers::read"]` — tap target 16×16px < 44px (mobile)
- 🟡 `overlay-create-user-drawer` [mobile] `[data-testid="user-permissions-tree-check-auth::session_settings::manage"]` — tap target 16×16px < 44px (mobile)
- 🟡 `overlay-create-user-drawer` [mobile] `[data-testid="user-permissions-tree-check-auth::session_settings::read"]` — tap target 16×16px < 44px (mobile)
- 🟡 `overlay-create-user-drawer` [mobile] `[data-testid="user-permissions-tree-check-branches::create"]` — tap target 16×16px < 44px (mobile)
- … +391 more (see JSONL)

### G7 (83)

- 🟡 `overlay-create-user-drawer` [desktop] `[data-testid="user-permissions-tree-check-citations::manage"]` — focus ring (3px) clipped by overflow-y ancestor div.max-h-80.overflow-auto — cut 19px
- 🟡 `overlay-create-user-drawer` [desktop] `[data-testid="user-permissions-tree-check-file_rag::admin::read"]` — focus ring (3px) clipped by overflow-y ancestor div.max-h-80.overflow-auto — cut 19px
- 🟡 `overlay-create-user-drawer` [desktop] `[data-testid="user-permissions-tree-check-files::upload"]` — focus ring (3px) clipped by overflow-y ancestor div.max-h-80.overflow-auto — cut 19px
- 🟡 `overlay-create-user-drawer` [desktop] `[data-testid="user-permissions-tree-check-group:auth"]` — focus ring (3px) clipped by overflow-y ancestor div.max-h-80.overflow-auto — cut 20px
- 🟡 `overlay-create-user-drawer` [desktop] `[data-testid="user-permissions-tree-check-group:conversations"]` — focus ring (3px) clipped by overflow-y ancestor div.max-h-80.overflow-auto — cut 19px
- 🟡 `overlay-create-user-drawer` [desktop] `[data-testid="user-permissions-tree-check-group:hardware"]` — focus ring (3px) clipped by overflow-y ancestor div.max-h-80.overflow-auto — cut 19px
- 🟡 `overlay-create-user-drawer` [desktop] `[data-testid="user-permissions-tree-check-hub::assistants::refresh"]` — focus ring (3px) clipped by overflow-y ancestor div.max-h-80.overflow-auto — cut 19px
- 🟡 `overlay-create-user-drawer` [mobile] `[data-testid="user-permissions-tree-check-citations::manage"]` — focus ring (3px) clipped by overflow-y ancestor div.max-h-80.overflow-auto — cut 19px
- 🟡 `overlay-create-user-drawer` [mobile] `[data-testid="user-permissions-tree-check-file_rag::admin::read"]` — focus ring (3px) clipped by overflow-y ancestor div.max-h-80.overflow-auto — cut 19px
- 🟡 `overlay-create-user-drawer` [mobile] `[data-testid="user-permissions-tree-check-files::upload"]` — focus ring (3px) clipped by overflow-y ancestor div.max-h-80.overflow-auto — cut 19px
- 🟡 `overlay-create-user-drawer` [mobile] `[data-testid="user-permissions-tree-check-group:auth"]` — focus ring (3px) clipped by overflow-y ancestor div.max-h-80.overflow-auto — cut 20px
- 🟡 `overlay-create-user-drawer` [mobile] `[data-testid="user-permissions-tree-check-group:conversations"]` — focus ring (3px) clipped by overflow-y ancestor div.max-h-80.overflow-auto — cut 19px
- 🟡 `overlay-create-user-drawer` [mobile] `[data-testid="user-permissions-tree-check-group:hardware"]` — focus ring (3px) clipped by overflow-y ancestor div.max-h-80.overflow-auto — cut 19px
- 🟡 `overlay-create-user-drawer` [mobile] `[data-testid="user-permissions-tree-check-hub::assistants::refresh"]` — focus ring (3px) clipped by overflow-y ancestor div.max-h-80.overflow-auto — cut 19px
- 🟡 `overlay-create-user-drawer` [tablet] `[data-testid="user-permissions-tree-check-citations::manage"]` — focus ring (3px) clipped by overflow-y ancestor div.max-h-80.overflow-auto — cut 19px
- 🟡 `overlay-create-user-drawer` [tablet] `[data-testid="user-permissions-tree-check-file_rag::admin::read"]` — focus ring (3px) clipped by overflow-y ancestor div.max-h-80.overflow-auto — cut 19px
- 🟡 `overlay-create-user-drawer` [tablet] `[data-testid="user-permissions-tree-check-files::upload"]` — focus ring (3px) clipped by overflow-y ancestor div.max-h-80.overflow-auto — cut 19px
- 🟡 `overlay-create-user-drawer` [tablet] `[data-testid="user-permissions-tree-check-group:auth"]` — focus ring (3px) clipped by overflow-y ancestor div.max-h-80.overflow-auto — cut 20px
- 🟡 `overlay-create-user-drawer` [tablet] `[data-testid="user-permissions-tree-check-group:conversations"]` — focus ring (3px) clipped by overflow-y ancestor div.max-h-80.overflow-auto — cut 19px
- 🟡 `overlay-create-user-drawer` [tablet] `[data-testid="user-permissions-tree-check-group:hardware"]` — focus ring (3px) clipped by overflow-y ancestor div.max-h-80.overflow-auto — cut 19px
- 🟡 `overlay-create-user-drawer` [tablet] `[data-testid="user-permissions-tree-check-hub::assistants::refresh"]` — focus ring (3px) clipped by overflow-y ancestor div.max-h-80.overflow-auto — cut 19px
- 🟡 `overlay-edit-user-group-drawer` [desktop] `[data-testid="user-permissions-tree-check-citations::manage"]` — focus ring (3px) clipped by overflow-y ancestor div.max-h-80.overflow-auto — cut 19px
- 🟡 `overlay-edit-user-group-drawer` [desktop] `[data-testid="user-permissions-tree-check-file_rag::admin::read"]` — focus ring (3px) clipped by overflow-y ancestor div.max-h-80.overflow-auto — cut 19px
- 🟡 `overlay-edit-user-group-drawer` [desktop] `[data-testid="user-permissions-tree-check-files::upload"]` — focus ring (3px) clipped by overflow-y ancestor div.max-h-80.overflow-auto — cut 19px
- 🟡 `overlay-edit-user-group-drawer` [desktop] `[data-testid="user-permissions-tree-check-group:auth"]` — focus ring (3px) clipped by overflow-y ancestor div.max-h-80.overflow-auto — cut 20px
- 🟡 `overlay-edit-user-group-drawer` [desktop] `[data-testid="user-permissions-tree-check-group:conversations"]` — focus ring (3px) clipped by overflow-y ancestor div.max-h-80.overflow-auto — cut 19px
- 🟡 `overlay-edit-user-group-drawer` [desktop] `[data-testid="user-permissions-tree-check-group:hardware"]` — focus ring (3px) clipped by overflow-y ancestor div.max-h-80.overflow-auto — cut 19px
- 🟡 `overlay-edit-user-group-drawer` [desktop] `[data-testid="user-permissions-tree-check-hub::assistants::refresh"]` — focus ring (3px) clipped by overflow-y ancestor div.max-h-80.overflow-auto — cut 19px
- 🟡 `overlay-edit-user-group-drawer` [mobile] `[data-testid="user-permissions-tree-check-citations::manage"]` — focus ring (3px) clipped by overflow-y ancestor div.max-h-80.overflow-auto — cut 19px
- 🟡 `overlay-edit-user-group-drawer` [mobile] `[data-testid="user-permissions-tree-check-file_rag::admin::read"]` — focus ring (3px) clipped by overflow-y ancestor div.max-h-80.overflow-auto — cut 19px
- … +53 more (see JSONL)

### I1 (80)

- 🟡 `chat` [mobile] `[data-testid="chat-input-add-btn"]` — interactive <button> "" occluded at center by span.[&_svg]:size-4 (hit-test miss)
- 🟡 `deep-chat-attachments` [mobile] `[data-testid="chat-export-btn"]` — interactive <button> "Export" occluded at center by [data-testid="ullm-model-select"] (hit-test miss)
- 🟡 `deep-chat-mcp-toolcall-completed` [mobile] `[data-testid="chat-export-btn"]` — interactive <button> "Export" occluded at center by [data-testid="ullm-model-select"] (hit-test miss)
- 🟡 `deep-chat-mcp-toolcall-error` [mobile] `[data-testid="chat-export-btn"]` — interactive <button> "Export" occluded at center by [data-testid="ullm-model-select"] (hit-test miss)
- 🟡 `deep-chat-tool-failed` [mobile] `[data-testid="chat-export-btn"]` — interactive <button> "Export" occluded at center by [data-testid="ullm-model-select"] (hit-test miss)
- 🟡 `deep-chat-tool-running` [mobile] `[data-testid="chat-export-btn"]` — interactive <button> "Export" occluded at center by [data-testid="ullm-model-select"] (hit-test miss)
- 🟡 `overlay-create-user-drawer` [desktop] `[data-testid="user-permissions-tree-check-files::read"]` — interactive <span> "" occluded at center by div[data-slot=field].group/field.flex (hit-test miss)
- 🟡 `overlay-create-user-drawer` [desktop] `[data-testid="user-permissions-tree-check-group:file_rag"]` — interactive <span> "" occluded at center by div.flex.w-full (hit-test miss)
- 🟡 `overlay-create-user-drawer` [desktop] `[data-testid="user-permissions-tree-check-hub::models::refresh"]` — interactive <span> "" occluded at center by div.border-t.bg-muted/50 (hit-test miss)
- 🟡 `overlay-create-user-drawer` [desktop] `[data-testid="user-permissions-tree-check-lit_search::admin::manage"]` — interactive <span> "" occluded at center by div.border-t.bg-muted/50 (hit-test miss)
- 🟡 `overlay-create-user-drawer` [mobile] `[data-testid="user-permissions-tree-check-files::read"]` — interactive <span> "" occluded at center by div[data-slot=field].group/field.flex (hit-test miss)
- 🟡 `overlay-create-user-drawer` [mobile] `[data-testid="user-permissions-tree-check-group:file_rag"]` — interactive <span> "" occluded at center by div.flex.w-full (hit-test miss)
- 🟡 `overlay-create-user-drawer` [mobile] `[data-testid="user-permissions-tree-check-hub::models::refresh"]` — interactive <span> "" occluded at center by div.border-t.bg-muted/50 (hit-test miss)
- 🟡 `overlay-create-user-drawer` [tablet] `[data-testid="user-permissions-tree-check-files::read"]` — interactive <span> "" occluded at center by div[data-slot=field].group/field.flex (hit-test miss)
- 🟡 `overlay-create-user-drawer` [tablet] `[data-testid="user-permissions-tree-check-group:file_rag"]` — interactive <span> "" occluded at center by div.flex.w-full (hit-test miss)
- 🟡 `overlay-create-user-drawer` [tablet] `[data-testid="user-permissions-tree-check-group:llm_local_runtime"]` — interactive <span> "" occluded at center by div.border-t.bg-muted/50 (hit-test miss)
- 🟡 `overlay-create-user-drawer` [tablet] `[data-testid="user-permissions-tree-check-llm_local_runtime::delete"]` — interactive <span> "" occluded at center by div.border-t.bg-muted/50 (hit-test miss)
- 🟡 `overlay-dialog-host-bare` [desktop] `[data-testid="gallery-dialog-no-desc-ok-btn"]` — interactive <button> "OK" occluded at center by [data-testid="gallery-dialog-no-desc-ok-btn"] (hit-test miss)
- 🟡 `overlay-dialog-host-bare` [mobile] `[data-testid="gallery-dialog-no-desc-ok-btn"]` — interactive <button> "OK" occluded at center by [data-testid="gallery-dialog-no-desc-ok-btn"] (hit-test miss)
- 🟡 `overlay-dialog-host-bare` [tablet] `[data-testid="gallery-dialog-no-desc-ok-btn"]` — interactive <button> "OK" occluded at center by [data-testid="gallery-dialog-no-desc-ok-btn"] (hit-test miss)
- 🟡 `overlay-dialog-host-described` [desktop] `[data-testid="gallery-dialog-with-desc-ok-btn"]` — interactive <button> "OK" occluded at center by [data-testid="gallery-dialog-with-desc-ok-btn"] (hit-test miss)
- 🟡 `overlay-dialog-host-described` [mobile] `[data-testid="gallery-dialog-with-desc-ok-btn"]` — interactive <button> "OK" occluded at center by [data-testid="gallery-dialog-with-desc-ok-btn"] (hit-test miss)
- 🟡 `overlay-dialog-host-described` [tablet] `[data-testid="gallery-dialog-with-desc-ok-btn"]` — interactive <button> "OK" occluded at center by [data-testid="gallery-dialog-with-desc-ok-btn"] (hit-test miss)
- 🟡 `overlay-edit-user-group-drawer` [desktop] `[data-testid="user-permissions-tree-check-file_rag::admin::manage"]` — interactive <span> "" occluded at center by div.flex.w-full (hit-test miss)
- 🟡 `overlay-edit-user-group-drawer` [mobile] `[data-testid="user-permissions-tree-check-file_rag::admin::manage"]` — interactive <span> "" occluded at center by div.flex.w-full (hit-test miss)
- 🟡 `overlay-edit-user-group-drawer` [tablet] `[data-testid="user-permissions-tree-check-file_rag::admin::manage"]` — interactive <span> "" occluded at center by div.flex.w-full (hit-test miss)
- 🟡 `seeded-file-rag-error` [desktop] `input#_r_v_-field` — interactive <input> "" occluded at center by [data-testid="filerag-fts-switch"] (hit-test miss)
- 🟡 `seeded-file-rag-error` [tablet] `input#_r_v_-field` — interactive <input> "" occluded at center by [data-testid="filerag-fts-switch"] (hit-test miss)
- 🟡 `seeded-s2-project-files-manage-empty` [mobile] `input.sr-only` — interactive <input> "" occluded at center by div.flex.cursor-pointer (hit-test miss)
- 🟡 `seeded-s3-download-view-downloading` [desktop] `[data-testid="llm-download-branch-input"]` — interactive <input> "" occluded at center by div.border-t.bg-muted/50 (hit-test miss)
- … +50 more (see JSONL)

### J6 (3)

- 🟡 `chats` [mobile] `div.flex.items-center` — peer icon-only action group mixes button variants: {outline, default} — chat-history-search-toggle-btn=outline, chat-history-header-new-chat-btn=default
- 🟡 `chats` [mobile] `div.flex.items-center` — peer icon-only action group mixes button variants: {outline, default} — chat-history-search-toggle-btn=outline, chat-history-header-new-chat-btn=default
- 🟡 `chats` [mobile] `div.flex.items-center` — peer icon-only action group mixes button variants: {outline, default} — chat-history-search-toggle-btn=outline, chat-history-header-new-chat-btn=default

### J7 (37)

- 🟡 `chat` [desktop] `[data-testid="mcp-chip-e0000000-0000-0000-0000-0000000000e1-close"]` — "close" control on the right here but left in the majority of containers (52 left / 29 right) — inconsistent placement
- 🟡 `chat` [mobile] `[data-testid="mcp-chip-e0000000-0000-0000-0000-0000000000e1-close"]` — "close" control on the right here but left in the majority of containers (52 left / 29 right) — inconsistent placement
- 🟡 `chat` [tablet] `[data-testid="mcp-chip-e0000000-0000-0000-0000-0000000000e1-close"]` — "close" control on the right here but left in the majority of containers (52 left / 29 right) — inconsistent placement
- 🟡 `deep-chat-attachments` [desktop] `[data-testid="mcp-chip-e0000000-0000-0000-0000-0000000000e1-close"]` — "close" control on the right here but left in the majority of containers (52 left / 29 right) — inconsistent placement
- 🟡 `deep-chat-attachments` [mobile] `[data-testid="mcp-chip-e0000000-0000-0000-0000-0000000000e1-close"]` — "close" control on the right here but left in the majority of containers (52 left / 29 right) — inconsistent placement
- 🟡 `deep-chat-attachments` [tablet] `[data-testid="mcp-chip-e0000000-0000-0000-0000-0000000000e1-close"]` — "close" control on the right here but left in the majority of containers (52 left / 29 right) — inconsistent placement
- 🟡 `deep-chat-mcp-toolcall-completed` [desktop] `[data-testid="mcp-chip-e0000000-0000-0000-0000-0000000000e1-close"]` — "close" control on the right here but left in the majority of containers (52 left / 29 right) — inconsistent placement
- 🟡 `deep-chat-mcp-toolcall-completed` [mobile] `[data-testid="mcp-chip-e0000000-0000-0000-0000-0000000000e1-close"]` — "close" control on the right here but left in the majority of containers (52 left / 29 right) — inconsistent placement
- 🟡 `deep-chat-mcp-toolcall-completed` [tablet] `[data-testid="mcp-chip-e0000000-0000-0000-0000-0000000000e1-close"]` — "close" control on the right here but left in the majority of containers (52 left / 29 right) — inconsistent placement
- 🟡 `deep-chat-mcp-toolcall-error` [desktop] `[data-testid="mcp-chip-e0000000-0000-0000-0000-0000000000e1-close"]` — "close" control on the right here but left in the majority of containers (52 left / 29 right) — inconsistent placement
- 🟡 `deep-chat-mcp-toolcall-error` [mobile] `[data-testid="mcp-chip-e0000000-0000-0000-0000-0000000000e1-close"]` — "close" control on the right here but left in the majority of containers (52 left / 29 right) — inconsistent placement
- 🟡 `deep-chat-mcp-toolcall-error` [tablet] `[data-testid="mcp-chip-e0000000-0000-0000-0000-0000000000e1-close"]` — "close" control on the right here but left in the majority of containers (52 left / 29 right) — inconsistent placement
- 🟡 `deep-chat-tool-failed` [desktop] `[data-testid="mcp-chip-e0000000-0000-0000-0000-0000000000e1-close"]` — "close" control on the right here but left in the majority of containers (52 left / 29 right) — inconsistent placement
- 🟡 `deep-chat-tool-failed` [mobile] `[data-testid="mcp-chip-e0000000-0000-0000-0000-0000000000e1-close"]` — "close" control on the right here but left in the majority of containers (52 left / 29 right) — inconsistent placement
- 🟡 `deep-chat-tool-failed` [tablet] `[data-testid="mcp-chip-e0000000-0000-0000-0000-0000000000e1-close"]` — "close" control on the right here but left in the majority of containers (52 left / 29 right) — inconsistent placement
- 🟡 `deep-chat-tool-running` [desktop] `[data-testid="mcp-chip-e0000000-0000-0000-0000-0000000000e1-close"]` — "close" control on the right here but left in the majority of containers (52 left / 29 right) — inconsistent placement
- 🟡 `deep-chat-tool-running` [mobile] `[data-testid="mcp-chip-e0000000-0000-0000-0000-0000000000e1-close"]` — "close" control on the right here but left in the majority of containers (52 left / 29 right) — inconsistent placement
- 🟡 `deep-chat-tool-running` [tablet] `[data-testid="mcp-chip-e0000000-0000-0000-0000-0000000000e1-close"]` — "close" control on the right here but left in the majority of containers (52 left / 29 right) — inconsistent placement
- 🟡 `seeded-login-error` [desktop] `[data-testid="auth-login-error-close"]` — "close" control on the right here but left in the majority of containers (52 left / 29 right) — inconsistent placement
- 🟡 `seeded-login-error` [mobile] `[data-testid="auth-login-error-close"]` — "close" control on the right here but left in the majority of containers (52 left / 29 right) — inconsistent placement
- 🟡 `seeded-login-error` [tablet] `[data-testid="auth-login-error-close"]` — "close" control on the right here but left in the majority of containers (52 left / 29 right) — inconsistent placement
- 🟡 `seeded-register-error` [desktop] `[data-testid="auth-register-error-close"]` — "close" control on the right here but left in the majority of containers (52 left / 29 right) — inconsistent placement
- 🟡 `seeded-register-error` [mobile] `[data-testid="auth-register-error-close"]` — "close" control on the right here but left in the majority of containers (52 left / 29 right) — inconsistent placement
- 🟡 `seeded-register-error` [tablet] `[data-testid="auth-register-error-close"]` — "close" control on the right here but left in the majority of containers (52 left / 29 right) — inconsistent placement
- 🟡 `seeded-s3-download-view-downloading` [desktop] `[data-testid="llm-download-drawer-close-btn"]` — "close" control on the right here but left in the majority of containers (52 left / 29 right) — inconsistent placement
- 🟡 `seeded-s3-download-view-downloading` [tablet] `[data-testid="llm-download-drawer-close-btn"]` — "close" control on the right here but left in the majority of containers (52 left / 29 right) — inconsistent placement
- 🟡 `seeded-s3-download-view-failed` [desktop] `[data-testid="llm-download-drawer-close-btn"]` — "close" control on the right here but left in the majority of containers (52 left / 29 right) — inconsistent placement
- 🟡 `seeded-s3-download-view-failed` [mobile] `[data-testid="llm-download-drawer-close-btn"]` — "close" control on the right here but left in the majority of containers (52 left / 29 right) — inconsistent placement
- 🟡 `seeded-s3-download-view-failed` [tablet] `[data-testid="llm-download-drawer-close-btn"]` — "close" control on the right here but left in the majority of containers (52 left / 29 right) — inconsistent placement
- 🟡 `seeded-s4-project-bib-manage-empty` [mobile] `button.ml-1.inline-flex` — "copy" control on the right here but left in the majority of containers (46 left / 8 right) — inconsistent placement
- … +7 more (see JSONL)
