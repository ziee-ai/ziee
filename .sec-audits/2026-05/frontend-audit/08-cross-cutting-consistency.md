# Cross-cutting consistency audit

Scope: Cross-cutting *consistency* across all `src-app/ui/src/modules/`.
Audits whether the same UI pattern is implemented the same way across
modules. Per-module logic is out of scope (covered by Agents 1-7).

The wrapper drawer `modules/layouts/app-layout/components/Drawer.tsx`
defaults to `size=520`, gracefully translates `width` to `size` for
backwards-compat, and switches to `width: '100%'` on `xs` viewports.
Operators authoring forms are *expected* to use this wrapper. Every
finding below measures divergence from this baseline (or, in pattern 5+,
from the closest thing to a baseline that the codebase has).

---

## Summary

Top inconsistencies, by impact:

1. **I-1 (HIGH)** — Form footer button **order** is split roughly
   50/50 between "Submit-then-Cancel" (user/group flows) and
   "Cancel-then-Submit" (everywhere else). Same workflow, opposite
   button order across modules.
2. **I-2 (HIGH)** — Submit-button **label** is ad-hoc: "Create User",
   "Add Provider", "Update Group", "Add", "Save", "Save Changes",
   "Done", "Start Download", "Upload", "Update Server", etc. No
   convention.
3. **I-3 (HIGH)** — Permission-gated read-only forms are implemented
   *only* in code-sandbox (Plan 6's canonical pattern). Every other
   admin form (LLM provider, MCP server, user, group, repository,
   assistant) silently lets users *open* an edit drawer and submit
   even when they lack `manage` permission; the failure surfaces only
   as an API error.
4. **I-4 (HIGH)** — Destructive-action confirmation is split between
   `modal.confirm(...)` (3 sites) and `<Popconfirm>` (10 sites).
   Different confirmation UX for delete on different surfaces. Some
   destructive switches/buttons have *no* confirmation at all
   (toggle-active uses Popconfirm; rotate API key has none).
5. **I-5 (MED)** — Drawer widths: dominant value is `size={600}` (15
   instances) with the wrapper default of `520`. Two drawers use
   `size={400}` (Groups list, Group Members), `RuntimeDownloadDrawer`
   uses the *deprecated* `width={500}` AND imports `Drawer` directly
   from antd bypassing the project wrapper. `UserGroupsSettings`
   inline-drawer uses `width={600}` (deprecated alias).
6. **I-6 (MED)** — Empty-state copy diverges: "No X found" / "No X
   yet" / "No X added yet" / "No X configured" / "No X available" /
   "No X assigned" — six phrasings for the same concept. Empty *UI*
   diverges too: some use `<Empty>` (with/without illustration), some
   use `<Text type="secondary">` plain, some use centered-`<Title>`
   with CTA, one uses a custom CSS spinner.
7. **I-7 (MED)** — Layout primitives are three-way split:
   `<Flex>` (Ant) — 38 files; Tailwind `className="flex ..."` — 78
   files; `<Space>` (Ant) — 24 files. No internal convention; widgets
   in the same module mix two of the three.

Secondary patterns covered: spacing-value divergence (`gap-2` /
`gap-3` / `gap-4` all common), repeated magic numbers in inline
styles (`fontSize: '12px'` × 43, `width: 120` × 36, `marginBottom: 16`
× 14), icon-library split (`@ant-design/icons` 102 files +
`react-icons/*` 17 files — no lucide).

---

## Inconsistencies (I-N)

### I-1 — Form footer button order [HIGH]

Two distinct conventions co-exist in the codebase:

- **"Submit, then Cancel"** (left-aligned, `<Flex className="gap-2">`):
  `CreateUserDrawer.tsx:120-133`, `EditUserDrawer.tsx:130-143`,
  `ResetPasswordDrawer.tsx:69-83`, `AssignGroupDrawer.tsx:61-74`.
  Six total in the `user` module.
- **"Cancel, then Submit"** (right-aligned,
  `<div className="flex justify-end gap-3 pt-4">` or `gap-2`):
  `AssistantFormDrawer.tsx:240-247`, `EditUserGroupDrawer.tsx:146-153`,
  `LlmProviderDrawer.tsx:172-179`, `LlmRepositoryDrawer.tsx:367-380`,
  `McpServerDrawer.tsx:525-530`,
  `LlmProviderGroupsAssignmentDrawer.tsx:112-124`,
  `GroupLlmProvidersAssignmentDrawer.tsx:80-93`,
  `McpServerGroupsAssignmentDrawer.tsx:93-106`,
  `GroupSystemMcpServersAssignmentDrawer.tsx:81-94`.

The user module is the outlier (Submit-then-Cancel, no `justify-end`).
Same workflow, opposite button order across modules — guaranteed user
confusion. Pick one and apply universally.

### I-2 — Submit-button label is ad-hoc [HIGH]

Labels found:

| Drawer | Label |
|---|---|
| `CreateUserDrawer.tsx:121` | "Create User" |
| `EditUserDrawer.tsx:133` | "Update User" |
| `ResetPasswordDrawer.tsx:72` | "Reset Password" |
| `AssignGroupDrawer.tsx:64` | "Assign Group" |
| `EditUserGroupDrawer.tsx:151` | "Update Group" |
| `UserGroupsSettings.tsx` (inline create) | "Create Group" |
| `LlmProviderDrawer.tsx:177` | "Add Provider" / "Update Provider" |
| `LlmRepositoryDrawer.tsx:379` | "Add Repository" / "Update Repository" |
| `AssistantFormDrawer.tsx:244-245` | "Create" / "Update" (no object) |
| `McpServerDrawer.tsx` (`getButtonText():299-311`) | "Create Server" / "Update Server" / "Save" |
| `EditLlmModelDrawer.tsx:100` | "Save Changes" |
| `AddRemoteLlmModelDrawer.tsx:83` | "Add" |
| `AddLocalLlmModelDownloadDrawer.tsx:247` | "Start Download" |
| `AddLocalLlmModelUploadDrawer.tsx:388` | "Upload" / "Uploading..." |
| `RuntimeDownloadDrawer.tsx:55` | "Download" |
| `LlmProviderGroupsAssignmentDrawer.tsx:122` | "Save" |
| Other group/server assignment drawers | "Save" |

No convention: some say "Save", some "Save Changes", some
verb-noun ("Create User"), some bare verb ("Create"), one says
"Done" (`McpServerDrawer` default fallthrough). A simple convention
("Create X" / "Save changes" / "Cancel") would be a one-day fix.

### I-3 — Permission-aware read-only forms only in code-sandbox [HIGH]

Plan 6 (security remediation) introduced the canonical pattern of
disabling the entire `<Form>` and hiding the submit button when the
viewer lacks `manage` permission. Implementation status:

- **Implemented**:
  `SandboxResourceLimitsSection.tsx:158` (`<Form disabled={!canManage}>`)
  + submit disabled at line 428;
  `SandboxEnvironmentsSection.tsx:168, 190` (per-button disable +
  tooltip).
- **Not implemented** (audit drawer files):
  `LlmProviderDrawer.tsx`, `LlmRepositoryDrawer.tsx`,
  `McpServerDrawer.tsx`, `EditLlmModelDrawer.tsx`,
  `AssistantFormDrawer.tsx`, every drawer in `modules/user/`,
  every drawer in `modules/mcp/components/system/`, every assignment
  drawer in `llm-provider/components/`.

`grep -rln "hasPermission"` returns *only* the two code-sandbox files.
Every other admin module silently lets a user without `manage` permission
open the drawer, edit fields, click submit, and only learn it failed via
a server 403. Plan 6's pattern must be propagated to every admin form.

Note: the codebase has no shared `hasPermission` helper — code-sandbox
defines its own at
`SandboxEnvironmentsSection.tsx:17`. When propagating the pattern,
hoist this helper to `core/permissions.ts` first.

### I-4 — Destructive-action confirmation is split [HIGH]

| Pattern | Sites |
|---|---|
| `modal.confirm({...})` | `McpServerCard.tsx:37`, `ProviderHeader.tsx:86`, `AssistantCard.tsx:119` |
| `<Popconfirm>` | `ConversationCard.tsx`, `LlmRepositorySettings.tsx:142`, `ConversationList.tsx`, `GroupListItem.tsx`, `UsersSettings.tsx:88, 133`, `RecentConversationsWidget.tsx`, `SandboxEnvironmentsSection.tsx:151`, `UserGroupsDrawer.tsx:125`, `RuntimeVersionCard.tsx`, `AssistantsSettings.tsx` |
| **No confirmation** | several toggle-active switches, "remove from group" via Tag-close, etc. |

Different UX for the same intent across modules. Modal confirms have
title + content + okType, Popconfirms have title only — different
information density. Pick one (Popconfirm for in-place destructive
actions, Modal for navigation-affecting deletes) and document it.

Bonus: cancel/OK labels are `okText="Yes"/cancelText="No"`
(`UsersSettings.tsx:92`) versus `okText="Delete"/cancelText="Cancel"`
(`LlmRepositorySettings.tsx:146`) — even within the same pattern.

### I-5 — Drawer widths [MED]

15 drawers use `size={600}`, 2 use `size={400}`, 1 uses the deprecated
`width={500}` (also bypassing wrapper), 1 uses deprecated `width={600}`.
The default `size=520` (wrapper) is *not used* by any consumer — every
caller overrides. Either (a) update the wrapper default to 600, or (b)
document the size-selection rationale (it is currently arbitrary).

The `RuntimeDownloadDrawer.tsx:2,46-58` imports `Drawer` directly from
`antd`, so it bypasses all the wrapper's responsive logic, mobile
sizing, and styles — see R-1 below.

### I-6 — Empty-state copy + UI diverge [MED]

See Appendix 5 for the full table. Six phrasings ("No X yet" /
"No X found" / "No X added yet" / "No X configured" / "No X available"
/ "No X assigned"), three UI styles (`<Empty>` with image, plain
`<Text type="secondary">`, full-page `<Title>`+CTA). The hub tabs use
plain `<Text>` while the settings pages use `<Empty>` — same kind of
data, different empty UX.

### I-7 — Layout primitives three-way split [MED]

| Primitive | File count |
|---|---|
| Tailwind `className="flex …"` etc. | 78 |
| Ant `<Flex>` | 38 |
| Ant `<Space>` | 24 |

Same module (e.g. `llm-provider/components/`) uses two of three.
Pick one for new code and migrate gradually. Tailwind has the highest
count and is the most flexible (gap, items-, justify-) — defaulting
to Tailwind and reserving `<Space>` for spaced lists of children
would be a clean rule.

### I-8 — Loading-state divergence [MED]

- `<Spin>`: 25 files (most common).
- Button `loading={...}` prop: 51 sites (universal).
- `<Skeleton>` (Ant): **zero usages** in `modules/`.
- Custom CSS spinner (`<div className="animate-spin ...">`):
  `ConversationList.tsx:192` (one site, deviates).
- `<LoadingOutlined spin>` (ant-design icon): `MessageList.tsx:41`.

No skeleton states anywhere — pages flash blank-then-content on every
load. Inconsistent with modern Ant Design guidance for list-heavy
pages (users, providers, MCP servers).

### I-9 — Error-display divergence [MED]

- `message.error(...)`: 42 files (dominant).
- `notification.error(...)`: 0 files.
- `<Alert type="error">`: 7 files (mostly form-level errors).
- Inline `<Text type="danger">`: 11 files (mostly per-row download
  errors, validation hints).

`message.error` is the de facto standard but no helper centralises
error-message-formatting; every call site writes
`message.error(error?.message || 'Failed to X')`. Recommend a
`core/errors.ts` `reportError(err, fallback)` helper that formats
the API error shape consistently.

### I-10 — Icon library split [LOW]

- `@ant-design/icons`: 102 files (dominant).
- `react-icons/*` (bs / cg / fa / go / io / md / pi / ri / si): 17
  files.
- `lucide-react`: not in `modules/` (confirmed).
- Custom inline `<svg>`: not found in `modules/`.

`react-icons` is mostly the `io` subset
(`IoIosArrowBack`, `IoIosArrowDown`, `IoIosArrowForward`) and a few
provider-brand glyphs (`RiOpenaiFill`, `RiAnthropicFill`,
`RiGeminiFill`, `SiHuggingface`) that don't exist in
`@ant-design/icons`. The arrow icons can be swapped for
`ArrowLeftOutlined` / `DownOutlined` / `RightOutlined` to drop a
dependency; the brand glyphs are genuinely needed.

### I-11 — Module structure deviations [LOW]

See Appendix 9. Eleven of 20 modules deviate from the documented
`module.tsx + types.ts + stores/ + events/ + components/ + pages/`
layout. The `user` module has no `types.ts` (uses
`types/` directory instead). `chat`, `code-sandbox`, `llm-local-runtime`,
`llm-provider`, `llm-repository`, `mcp`, `router`, `onboarding` have
no `pages/` (logic lives in `components/`). `projects/`,
`user-llm-providers/`, `hardware/`, `auth/` are flat (no
subdirectories). `hub/` uses a nested-module structure
(`hub/modules/llm-models/`, `hub/modules/assistants/`,
`hub/modules/mcp/`) that no other module follows.

Most deviations are cosmetic, but the lack of a `pages/` boundary in
many modules makes it hard to distinguish page-level entries from
reusable components.

---

## Inefficiencies (E-N)

### E-1 — `Form.useForm()` + manual reset duplicates state lifecycle [LOW]

Every drawer (16 sites) writes
`const [form] = Form.useForm()` + `form.resetFields()` in `onClose` +
`form.setFieldsValue(...)` in a `useEffect`. This is correct but
boilerplate-heavy. A `useDrawerForm({ open, initialValues })` hook in
`core/` could centralise lifecycle. Not a critical efficiency loss,
but a maintenance cost compounding across ~20 drawer files.

### E-2 — Duplicate `validatePermissions` helper [LOW]

`CreateUserDrawer.tsx:10-35` and `EditUserDrawer.tsx:10-36` define
identical 25-line `validatePermissions` functions. Same with
`UserGroupsSettings.tsx` (third copy). Hoist to a util.

---

## Responsive / sizing / scrolling (R-N)

### R-1 — `RuntimeDownloadDrawer` imports antd `Drawer` directly [MED]

`modules/llm-local-runtime/components/drawers/RuntimeDownloadDrawer.tsx:2`:

```ts
import { Button, Drawer, Form, Input, message, Select, Space } from 'antd'
```

Bypasses the project's wrapper at
`modules/layouts/app-layout/components/Drawer.tsx`. Loses:
- Mobile responsive `width: '100%'` on `xs` viewports.
- Wrapper's resize-handle.
- Wrapper's `--bg-layout` styling on body/header/footer.
- Wrapper's title-bar with `<IoIosArrowBack>` close button.

On mobile this drawer will render at `width: 500` (overflowing a
375 px iPhone viewport). Replace import with the wrapper.

### R-2 — Drawers with `size={400}` may overflow on very narrow viewports [LOW]

`UserGroupsDrawer.tsx:94` and `GroupMembersDrawer.tsx:28` are 400 px
fixed. On `xs` the wrapper switches to `width: '100%'` (line 63 of
wrapper), so this is fine. Verified by inspection of
`Drawer.tsx:62-63`. No action.

### R-3 — Inline magic `width: 120` repeats 36× in one file [LOW]

`LlmModelLlamaCppSettingsSection.tsx` uses
`style={{ margin: 0, width: 120 }}` on 19 form items (lines 49, 68, 87,
106, 125, 177, 196, 263, 282, 301, 324, 341, 360, 380, 403, 427, 454,
473 — see Appendix 4). The repetition is benign but the magic
value is brittle — a single shared CSS class
(`.llama-cpp-input { width: 120px; margin: 0; }`) or Tailwind
`w-30` would be cleaner. Note: at very narrow viewports these
fixed-width inputs may not fit the form column.

---

## Appendix 1: Drawer widths

| File:line | Component | Width | Module |
|---|---|---|---|
| `modules/layouts/app-layout/components/Drawer.tsx:27` | wrapper default | `520` | layouts |
| `modules/mcp/components/common/McpServerDrawer.tsx:317` | McpServerDrawer | `size={600}` | mcp |
| `modules/mcp/components/system/McpServerGroupsAssignmentDrawer.tsx:92` | McpServerGroupsAssignmentDrawer | `size={600}` | mcp |
| `modules/mcp/components/system/GroupSystemMcpServersAssignmentDrawer.tsx:80` | GroupSystemMcpServersAssignmentDrawer | `size={600}` | mcp |
| `modules/llm-provider/components/LlmProviderDrawer.tsx:97` | LlmProviderDrawer | `size={600}` | llm-provider |
| `modules/llm-provider/components/LlmProviderGroupsAssignmentDrawer.tsx:110` | LlmProviderGroupsAssignmentDrawer | `size={600}` | llm-provider |
| `modules/llm-provider/components/GroupLlmProvidersAssignmentDrawer.tsx:79` | GroupLlmProvidersAssignmentDrawer | `size={600}` | llm-provider |
| `modules/llm-provider/components/llm-models/EditLlmModelDrawer.tsx:103` | EditLlmModelDrawer | `size={600}` | llm-provider |
| `modules/llm-provider/components/llm-models/AddRemoteLlmModelDrawer.tsx:86` | AddRemoteLlmModelDrawer | `size={600}` | llm-provider |
| `modules/llm-provider/components/llm-models/AddLocalLlmModelDownloadDrawer.tsx:251` | AddLocalLlmModelDownloadDrawer | `size={600}` | llm-provider |
| `modules/llm-provider/components/llm-models/AddLocalLlmModelUploadDrawer.tsx:391` | AddLocalLlmModelUploadDrawer | `size={600}` | llm-provider |
| `modules/llm-repository/components/LlmRepositoryDrawer.tsx:193` | LlmRepositoryDrawer | `size={600}` | llm-repository |
| `modules/user/components/user/CreateUserDrawer.tsx:75` | CreateUserDrawer | `size={600}` | user |
| `modules/user/components/user/EditUserDrawer.tsx:91` | EditUserDrawer | `size={600}` | user |
| `modules/user/components/group/EditUserGroupDrawer.tsx:105` | EditUserGroupDrawer | `size={600}` | user |
| `modules/user/components/group/UserGroupsSettings.tsx:184` | (inline create) | `width={600}` *(deprecated alias)* | user |
| `modules/assistants/components/AssistantFormDrawer.tsx:155` | AssistantFormDrawer | `size={600}` | assistants |
| `modules/user/components/user/UserGroupsDrawer.tsx:94` | UserGroupsDrawer | `size={400}` | user |
| `modules/user/components/group/GroupMembersDrawer.tsx:28` | GroupMembersDrawer | `size={400}` | user |
| `modules/llm-local-runtime/components/drawers/RuntimeDownloadDrawer.tsx:50` | RuntimeDownloadDrawer | `width={500}` *(deprecated + bypasses wrapper)* | llm-local-runtime |
| `modules/user/components/user/ResetPasswordDrawer.tsx:26` | ResetPasswordDrawer | *(default 520)* | user |
| `modules/user/components/user/AssignGroupDrawer.tsx:28` | AssignGroupDrawer | *(default 520)* | user |
| `modules/hub/modules/assistants/components/AssistantDetailsDrawer.tsx:21` | AssistantDetailsDrawer | *(default 520)* | hub |
| `modules/hub/modules/llm-models/components/ModelDetailsDrawer.tsx:22` | ModelDetailsDrawer | *(default 520)* | hub |
| `modules/hub/modules/mcp/components/McpServerDetailsDrawer.tsx:22` | McpServerDetailsDrawer | *(default 520)* | hub |

Dominant value: `size={600}` (15 explicit + 1 deprecated alias).
Default `520` only used by detail/read-only drawers in hub. Bot
divergence + the bypass at `RuntimeDownloadDrawer:50` are the flags.

---

## Appendix 2: Form footers

| File:line | Order | Submit label | Loading shape |
|---|---|---|---|
| `CreateUserDrawer.tsx:120-133` | Submit, Cancel (`<Flex>`) | "Create User" | `<Button loading={creatingUser}>` |
| `EditUserDrawer.tsx:130-143` | Submit, Cancel (`<Flex>`) | "Update User" | none on submit; relies on store |
| `ResetPasswordDrawer.tsx:69-83` | Submit, Cancel (`<Flex>`) | "Reset Password" | none |
| `AssignGroupDrawer.tsx:61-74` | Submit, Cancel (`<Flex>`) | "Assign Group" | none |
| `EditUserGroupDrawer.tsx:146-153` | Cancel, Submit (`justify-end`) | "Update Group" | `loading={loading}`, Cancel disabled |
| `UserGroupsSettings.tsx` (inline) | Submit, Cancel (`<Flex>`) | "Create Group" | none |
| `LlmProviderDrawer.tsx:172-179` | Cancel, Submit (`justify-end`) | "Add Provider" / "Update Provider" | `loading={loading}`, Cancel disabled |
| `LlmRepositoryDrawer.tsx:367-380` | Cancel, Submit (`justify-end`) | "Add Repository" / "Update Repository" | `loading={loading \|\| creating \|\| updating}` |
| `AssistantFormDrawer.tsx:240-247` | Cancel, Submit (`justify-end`) | "Create" / "Update" | `loading={loading}`, Cancel disabled |
| `McpServerDrawer.tsx:525-530` | Cancel, Submit (`justify-end`) | "Create Server" / "Update Server" / "Save" | `loading={loading}` |
| `EditLlmModelDrawer.tsx:90-102` (drawer-footer array) | Cancel, Submit | "Save Changes" | `loading={loading}` |
| `AddRemoteLlmModelDrawer.tsx:73-85` (drawer-footer array) | Cancel, Submit | "Add" | `loading={loading}` |
| `AddLocalLlmModelDownloadDrawer.tsx:237-249` (drawer-footer array) | Cancel, Submit | "Start Download" | `loading={loading}` |
| `AddLocalLlmModelUploadDrawer.tsx:377-389` (drawer-footer array) | Cancel, Submit | "Upload" / "Uploading..." | `loading={loading}`, both disabled while `uploading` |
| `RuntimeDownloadDrawer.tsx:51-58` (footer prop) | Cancel, Submit (`<Space>`) | "Download" | `loading={submitting}` |
| `LlmProviderGroupsAssignmentDrawer.tsx:112-124` | Cancel, Submit (`justify-end`) | "Save" | `loading={saving}`, submit `disabled={loading}` |
| `GroupLlmProvidersAssignmentDrawer.tsx:80-93` | Cancel, Submit (`justify-end`) | "Save" | `loading={saving}` |
| `McpServerGroupsAssignmentDrawer.tsx:93-106` | Cancel, Submit (`justify-end`) | "Save" | `loading={saving}` |
| `GroupSystemMcpServersAssignmentDrawer.tsx:81-94` | Cancel, Submit (`justify-end`) | "Save" | `loading={saving}` |

Footer-rendering technique also splits: `<Flex>` inside Form.Item
(user module), bare `<div className="flex justify-end">` after
`</Form>` (most others), wrapper's `footer` prop with an *array*
(llm-models drawers), wrapper's `footer` prop with a single JSX
node (assignment drawers), or `<Space>` (RuntimeDownloadDrawer).

---

## Appendix 3: Spacing frequencies

### Tailwind classes (occurrences)

| Class | Count | Note |
|---|---|---|
| `gap-2` | 128 | dominant horizontal gap |
| `gap-3` | 64 | secondary |
| `gap-1` | 37 | tight |
| `gap-4` | 11 | wider |
| `gap-6` | 6 | rare |
| `p-2` | 138 | dominant padding |
| `p-3` | 74 | secondary |
| `p-1` | 45 | tight |
| `p-4` | 23 | rare |
| `p-6` | 10 | wide |
| `p-8` | 8 | extra-wide |
| `px-3` | 33 | horizontal |
| `py-8` | 13 | empty-state pad |
| `py-12` | 11 | empty-state pad |
| `mb-2` | 39 | most common margin |
| `mb-4` | 27 | secondary |
| `m-0` | 25 | reset |
| `mt-2` | 18 | top margin |
| `mb-0` | 15 | reset |
| `space-y-2` | 6 | rare |
| `space-x-N` | 0 | not used |

### Ant `<Space>` (occurrences)

| Form | Count |
|---|---|
| `<Space size="small">` | 7 (mostly file-viewer headers) |
| `<Space direction="vertical" size="middle">` | 4 (in assignment drawers) |
| `<Space direction="vertical" size="large">` | 4 (in assignment drawers) |
| `<Space direction="vertical" size="small">` | 3 (in widgets) |

### Inline `style={{ ... }}` spacing (occurrences)

| Style | Count |
|---|---|
| `margin: 0` | 134 (mostly `Title style={{ margin: 0 }}`) |
| `marginBottom: 16` | 14 (Card spacers, mostly code-sandbox) |
| `marginTop: 8` | 9 |
| `marginBottom: 0` | 9 |
| `padding: '50px'` | 3 |
| `marginBottom: 8` | 3 |
| `marginTop: 16` | 2 |
| `padding: 0` | 4 |

Findings:
- No unified spacing token system. Authors split between `gap-2` and
  `gap-3` for the same intent ("small horizontal gap").
- `marginBottom: 16` only in code-sandbox cards — should migrate to
  `mb-4` (= 16 px in Tailwind default) when standardising.
- `<Space>` use is islanded to 4 patches: file-viewer headers,
  assignment drawers, widgets, RuntimeDownloadDrawer.

---

## Appendix 4: Magic numbers in inline styles

| Magic value | Occurrences | Modules / files |
|---|---|---|
| `fontSize: '12px'` | 43 | hardware (HardwareMonitor 14×), user (UsersSettings 2× + GroupListItem 2×), llm-provider (AddLocalLlmModelDownloadDrawer 2× + assignment drawers 3×), mcp (assignment drawers 2×), hub (cards), code-sandbox (table cell labels) |
| `width: 120` | 36 | llm-provider/components/llm-models/shared/LlmModelLlamaCppSettingsSection.tsx (19×, see R-3) + LlmModelMistralRsSettingsSection.tsx |
| `fontSize: '11px'` | 20 | tag styles in assignment drawers (LLM provider, MCP server, user-group) — `<Tag style={{ fontSize: '11px', margin: 0 }}>` |
| `marginBottom: 16` | 14 | code-sandbox sections (`marginBottom: 16` on Cards) |
| `marginTop: 8` | 9 | llm-local-runtime, llm-provider download cards |
| `width: '100%'` | 102 | dominantly correct usage (`<Space style={{ width: '100%' }}>` to expand) |
| `fontSize: '14px'` | 9 | assignment drawers `<Text strong style={{ fontSize: '14px' }}>` |
| `fontSize: '24px'` | 7 | hub cards, RecentConversationsWidget headers |
| `padding: '50px'` | 3 | loading/empty container padders |
| `padding: '6px 12px 12px 12px'` | 1 | Drawer wrapper footer-style (correctly localised) |
| `minWidth: 120` | 4 | AssistantSelector + McpServersSettings select dropdowns |
| `minWidth: 180` | 3 | unspecified — narrow selectors |

The repeated `fontSize: '12px'` and `fontSize: '11px'` are not in the
design tokens — they're hardcoded everywhere. Ant Design's theme
already exposes `token.fontSizeSM` (12 px) and `token.fontSizeXS`
(typically 10 px). All 63 occurrences should read from the token.

---

## Appendix 5: Empty states

| Surface | File:line | Copy | UI |
|---|---|---|---|
| Users list | `UsersSettings.tsx:185` | "No users found" | `<Empty description=...>` |
| User-groups list | `UserGroupsSettings.tsx:138` | "No user groups found" | `<Empty>` |
| User-groups (within drawer) | `UserGroupsDrawer.tsx:115` | "No groups available" | `<Empty>` |
| LLM providers list | `LlmProviderSettings.tsx` (no `<Empty>` found in audit) | n/a | n/a — uses inline Card render or no empty |
| LLM models per provider | `LlmModelsSection.tsx:251` | "No models added yet" | `<Empty>` |
| LLM repositories | `LlmRepositorySettings.tsx:184-191` | "No repositories configured" + secondary "Add a repository to get started" | `<Empty>` with `<CloudDownloadOutlined>` icon |
| MCP servers (user) | `McpServersSettings.tsx:195-203` | "No MCP servers configured" / "No servers match your search criteria" | plain `<Text type="secondary">` in `py-12` div |
| MCP servers (system) | `SystemMcpServersPage.tsx:153` | "No servers match your search criteria" | plain `<Text>` |
| MCP groups-assigned widget | `GroupSystemMcpServersWidget.tsx:72` | "No servers assigned" | plain text |
| LLM provider groups-assigned widget | `LLMProviderGroupWidget.tsx:70` | "No providers assigned" | plain text |
| LLM provider group-card | `ProviderGroupAssignmentCard.tsx:65` | "No groups assigned" | `<Text type="secondary">` |
| MCP server group-card | `McpServerGroupsAssignmentCard.tsx:61` | "No groups assigned" | `<Empty description=...>` |
| Assistants settings | `AssistantsSettings.tsx:130` | "No assistants found" | `<Empty>` |
| User assistants page | `UserAssistantsPage.tsx:212-232` | "No assistants found" / "No assistants yet" + CTA | full-page `<Title level={3}>` + `<RobotOutlined>` + Create button |
| Hub - assistants tab | `AssistantsHubTab.tsx:168` | "No assistants found" | plain `<Text>` in `text-center py-12` |
| Hub - models tab | `ModelsHubTab.tsx:167` | "No models found" | plain `<Text>` |
| Hub - MCP tab | `McpServersHubTab.tsx` | (similar) | plain `<Text>` |
| Recent conversations widget | `RecentConversationsWidget.tsx:50-63` | "No conversations yet" | `<Empty>` with `<MessageOutlined>` icon, `text-xs` |
| Conversation list | `ConversationList.tsx:179-186` | "No conversations found matching your search" / "No chat history yet" | `<Empty image={Empty.PRESENTED_IMAGE_SIMPLE}>` |
| Tool dropdown | `McpConfigModal.tsx:250` | "No tools available" | `<Empty image={Empty.PRESENTED_IMAGE_SIMPLE}>` |
| Assistant menu | `AssistantMenuItem.tsx:45` | "No assistants available" | plain text |

Six distinct copy patterns ("found" / "yet" / "added yet" /
"configured" / "available" / "assigned") and three distinct UI
strategies. The `UserAssistantsPage` empty state is the only one with
a "Get started by creating an X" CTA — others rely on the page header's
"Add" button being visible.

---

## Appendix 6: Error display

| Pattern | Files / sites |
|---|---|
| `message.error(...)` (default) | 42 files; e.g. `LlmProviderDrawer.tsx:85`, `LlmRepositoryDrawer.tsx:175`, `McpServerCard.tsx:51`, `EditLlmModelDrawer.tsx`, `AssistantsSettings.tsx`, all user-module drawers via `App.useApp()` |
| `notification.error(...)` | **0 files** — pattern abandoned |
| `<Alert type="error" ...>` | 7 files: `UserLlmProvidersPage.tsx`, `SandboxEnvironmentsSection.tsx:208`, `SandboxResourceLimitsSection.tsx:140`, `ConversationPage.tsx`, `mcp/extension.tsx`, `ProviderApiKeyModal.tsx`, onboarding (`McpServersStep.tsx`, `ApiKeysStep.tsx`) |
| Inline `<Text type="danger">` | 11 files; e.g. `DownloadIndicatorWidget.tsx:50` `("display: block, marginTop: 8")`, `AddLocalLlmModelDownloadDrawer.tsx:258`, `LLMProviderGroupWidget.tsx`, `GroupSystemMcpServersWidget.tsx`, hub cards |
| Raw `console.error` only (no UI) | many; e.g. `CreateUserDrawer.tsx:61` ("Error is handled by the store") |

Canonical helper: none. `message.error(err?.message || 'Failed to X')`
is repeated verbatim in every drawer. Recommend a `core/errors.ts`
exposing `reportApiError(err, fallback?: string)` that handles the
standard API error envelope.

---

## Appendix 7: Loading states

| Pattern | Sites |
|---|---|
| `<Button loading={...}>` | 51 sites (universal for submit buttons) |
| `<Spin>` (centered in container) | 25 files — most common page/drawer loading |
| `<Spin tip="..."/>` (Ant) | 4 sites (SandboxResourceLimitsSection, UserGroupsDrawer, MessageList area) |
| `<Skeleton>` (Ant) | **0 files** — never used |
| Custom CSS spinner (`animate-spin rounded-full`) | `ConversationList.tsx:192` (single deviating instance) |
| `<LoadingOutlined spin>` (icon) | `MessageList.tsx:41` |

Recommendation: introduce `<Skeleton>` for list-heavy pages (Users,
LLM providers, MCP servers, Models, Assistants), and either remove
the custom `animate-spin` spinner in `ConversationList.tsx:192` or
hoist it to a shared component.

---

## Appendix 8: Confirmation modals

| File:line | Action | Pattern | Confirm text |
|---|---|---|---|
| `McpServerCard.tsx:37` | Delete MCP server | `modal.confirm` | "Delete Server" / "Are you sure you want to delete '{name}'? This action cannot be undone." / okText="Delete" |
| `ProviderHeader.tsx:86` | Delete provider | `modal.confirm` | "Confirm Deletion" / "Are you sure...{name}'?" / okText="Delete" |
| `AssistantCard.tsx:119` | Delete assistant | `modal.confirm` | "Delete Assistant" / "Are you sure?" / okText="Delete" |
| `UsersSettings.tsx:88` | Toggle user active | `<Popconfirm>` | "Deactivate/Activate this user?" / Yes/No |
| `UsersSettings.tsx:133` | Delete user | `<Popconfirm>` | "Are you sure you want to delete this user?" / Yes/No |
| `LlmRepositorySettings.tsx:142` | Delete repository | `<Popconfirm>` | "Are you sure?" / Delete/Cancel |
| `UserGroupsDrawer.tsx:125` | Remove from group | `<Popconfirm>` | "Remove user from this group?" / Yes/No |
| `GroupListItem.tsx` | Delete group | `<Popconfirm>` | (in module) |
| `ConversationCard.tsx`, `ConversationList.tsx`, `RecentConversationsWidget.tsx` | Delete conversation | `<Popconfirm>` | varied |
| `SandboxEnvironmentsSection.tsx:151` | Evict rootfs | `<Popconfirm>` | "Evict cached rootfs?" + description / okText="Evict" |
| `RuntimeVersionCard.tsx` | Delete runtime | `<Popconfirm>` | (similar) |
| `AssistantsSettings.tsx` | Delete (template) | `<Popconfirm>` | (similar) |

Inconsistent okText labeling ("Delete" vs "Yes") and inconsistent
description prose (some have detail, some say "Are you sure?").

---

## Appendix 9: Module structure deviations

| Module | module.tsx | types.ts | stores/ | events/ | components/ | pages/ | Deviation |
|---|---|---|---|---|---|---|---|
| `app/` | Y | — | — | — | — | — | Flat; only `App.store.ts` + `SetupPage.tsx` |
| `assistants/` | Y | Y | Y | Y | Y | Y | Reference layout (matches docs) |
| `auth/` | Y | — | — | — | — | — | Flat (LoginForm, AuthPage, AuthGuard at top level) |
| `chat/` | Y | Y | Y | — | Y | Y | No `events/`; has `core/`, `extensions/`, `widgets/` instead |
| `code-sandbox/` | Y | Y | Y | — | Y | — | No `events/`, no `pages/` |
| `config-client/` | Y | — | — | — | — | — | Flat; store + module only |
| `hardware/` | Y | Y | — | — | — | — | Flat (no subdirectories) |
| `hub/` | Y | Y | — | — | — | — | Uses *nested-module* layout (`hub/modules/...`) |
| `layouts/` | — | — | — | — | — | — | Layout helpers; not a real module |
| `llm-local-runtime/` | Y | Y | Y | Y | Y | — | No `pages/`; has `utils/` |
| `llm-provider/` | Y | Y | Y | Y | Y | — | No `pages/`; has `widgets/`, `icons/`, `constants/`, `constants.tsx` |
| `llm-repository/` | Y | Y | Y | Y | Y | — | No `pages/` |
| `mcp/` | Y | Y | Y | Y | Y | — | No `pages/`; has `widgets/` |
| `onboarding/` | Y | Y | Y | Y | — | — | No `components/`; has `guides/` instead |
| `projects/` | Y | — | — | — | — | — | Flat |
| `router/` | Y | Y | Y | — | Y | — | No `events/`, no `pages/` |
| `settings/` | Y | — | — | — | Y | — | No `types.ts`, no `stores/`, no `events/`, no `pages/`; has `types/` |
| `settings-general/` | Y | — | — | — | Y | — | No `types.ts`, no `stores/`, no `events/`, no `pages/` |
| `user/` | Y | — | Y | Y | Y | — | **No `types.ts`** (uses `types/` directory); no `pages/` |
| `user-llm-providers/` | Y | Y | — | — | — | — | Flat; one page + one store at top level |
| `user-profile/` | Y | — | — | — | — | — | Flat; one widget at top level |

Deviations (in rough decreasing severity):

1. `user/` has no `types.ts` (uses directory) — inconsistent with
   most other modules.
2. `hub/` has a nested-module layout that no other module follows.
3. Most modules have no `pages/` — page-level components live in
   `components/`.
4. `chat/` has unique `core/`, `extensions/`, `widgets/`
   subdirectories.
5. `settings/` and `settings-general/` are skeletal but functional.

This isn't necessarily wrong, but the documentation under
`.claude/META_FRAMEWORK_ARCHITECTURE.md` implies a canonical layout
that very few modules actually follow.
