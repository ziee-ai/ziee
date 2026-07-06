# Uncovered render branches (GENERATED)

> `node scripts/gallery-coverage.mjs` — the runtime branch-coverage proof (Part 2).
> An uncovered arm is a conditional-render fork NO gallery combo exercised. Rows are
> split into **state gaps** (the arm sits on a Part-1 named-state signal — the
> actionable queue: add a gallery entry that reaches it, or allow-list it in
> `coverage-allowlist.json` with a reason) and **generic branch forks** (prop
> variants / defensive defaults — the state axis is already gated by Part 1's tsc
> gate + the kit stories, so these are informational).

## Summary

- 626 instrumented surface files rendered.
- **2** STATE gaps not allow-listed — the actionable queue.
- 8592 generic branch forks not allow-listed (informational).

## State-level gaps (actionable)

### `modules/llm-provider/components/LocalProviderSettings.tsx`

| line | state | condition |
|---|---|---|
| 35 | loading | `if (!currentProvider && (loading \|\| !isInitialized)) {` |

### `modules/user-profile/UserProfileWidget.tsx`

| line | state | condition |
|---|---|---|
| 86 | loading | `if (isInitializing \|\| isLoading) {` |

## Generic branch forks (informational — top files by count)

| file | uncovered forks |
|---|---|
| `modules/mcp/components/common/McpServerDrawer.tsx` | 395 |
| `modules/chat/core/stores/Chat.store.ts` | 220 |
| `modules/mcp/stores/McpComposer.store.ts` | 206 |
| `modules/mcp/chat-extension/extension.tsx` | 151 |
| `modules/layouts/app-layout/AppLayout.tsx` | 142 |
| `modules/file/stores/File.store.ts` | 113 |
| `modules/hub/modules/llm-models/components/ModelHubCard.tsx` | 113 |
| `modules/chat/core/extensions/registry.tsx` | 111 |
| `modules/workflow/components/workflowElicitSchema.ts` | 108 |
| `modules/auth/Auth.store.ts` | 107 |
| `components/ui/kit/multi-select.tsx` | 106 |
| `modules/llm-provider/components/llm-models/AddLocalLlmModelDownloadDrawer.tsx` | 105 |
| `modules/llm-repository/components/LlmRepositoryDrawer.tsx` | 100 |
| `modules/workflow/components/WorkflowElicitForm.tsx` | 100 |
| `modules/hardware/HardwareSettings.tsx` | 99 |
| `modules/code-sandbox/components/_rootfsShared.tsx` | 97 |
| `modules/auth-providers/components/AuthProviderEditDrawer.tsx` | 91 |
| `modules/workflow/components/EditableArrayTable.tsx` | 91 |
| `components/ui/kit/tree.tsx` | 86 |
| `modules/layouts/app-layout/components/LeftSidebar.tsx` | 86 |
| `modules/llm-provider/components/llm-models/AddLocalLlmModelUploadDrawer.tsx` | 83 |
| `modules/mcp/components/McpConfigModal.tsx` | 80 |
| `modules/workflow/stores/WorkflowRun.store.ts` | 80 |
| `modules/mcp/chat-extension/components/ElicitationFormContent.tsx` | 75 |
| `modules/llm-provider/components/LlmModelsSection.tsx` | 74 |
| `modules/file/project-extension/components/ProjectFilesManagePanel.tsx` | 73 |
| `modules/projects/chat-extension/extension.tsx` | 72 |
| `modules/settings/SettingsPage.tsx` | 61 |
| `modules/layouts/app-layout/components/Drawer.tsx` | 60 |
| `modules/layouts/app-layout/components/ResizeHandle.tsx` | 60 |
