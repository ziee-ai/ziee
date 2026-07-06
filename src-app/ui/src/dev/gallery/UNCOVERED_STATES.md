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
- **26** STATE gaps not allow-listed — the actionable queue.
- 8607 generic branch forks not allow-listed (informational).

## State-level gaps (actionable)

### `components/ui/kit/dialog-host.tsx`

| line | state | condition |
|---|---|---|
| 94 | overlay | `<AlertDialog key={it.id} open onOpenChange={(o) => { if (!o) close(it, false) }}>` |

### `components/ui/kit/sheet.tsx`

| line | state | condition |
|---|---|---|
| 106 | loading | `? <div className="flex min-h-40 items-center justify-center"><Spinner label={loadingLabel ?? ''} /></div>` |

### `modules/assistant/chat-extension/components/AssistantMenuItem.tsx`

| line | state | condition |
|---|---|---|
| 37 | empty | `<div className="px-3 py-1.5 text-sm text-muted-foreground">` |

### `modules/chat/pages/ChatHistoryPage.tsx`

| line | state | condition |
|---|---|---|
| 143 | error | `<div className={cn('flex flex-1 flex-col w-full', nativeScroll ? '' : 'overflow-hidden')}>` |

### `modules/citations/project-extension/components/ProjectBibliographyManagePanel.tsx`

| line | state | condition |
|---|---|---|
| 74 | loading | `<Spin label="Loading" />` |

### `modules/file-rag/components/sections/ChunkingSection.tsx`

| line | state | condition |
|---|---|---|
| 128 | error | `<Alert data-testid="filerag-chunking-error-alert" tone="error" className="!mb-4" title={error \|\| validationError} />` |

### `modules/file/project-extension/components/ProjectFilesManagePanel.tsx`

| line | state | condition |
|---|---|---|
| 257 | empty | `<div className="flex flex-col mb-3 gap-2">` |

### `modules/hardware/HardwareMonitor.tsx`

| line | state | condition |
|---|---|---|
| 183 | empty | `{!currentUsage?.gpu_devices \|\|` |
| 184 | empty | `currentUsage.gpu_devices.length === 0 ? (` |

### `modules/hub/modules/llm-models/components/ModelHubCard.tsx`

| line | state | condition |
|---|---|---|
| 111 | empty | `if (localProviders.length === 0) {` |
| 693 | overlay | `model={showDetails ? model : null}` |

### `modules/llm-provider/components/llm-models/AddLocalLlmModelDownloadDrawer.tsx`

| line | state | condition |
|---|---|---|
| 159 | empty | `if (res.files.length === 0) {` |
| 359 | overlay | `title={viewMode ? 'View Download Details' : 'Download from Repository'}` |

### `modules/llm-provider/components/llm-models/AddLocalLlmModelUploadDrawer.tsx`

| line | state | condition |
|---|---|---|
| 230 | empty | `if (selectedFiles.length === 0) {` |

### `modules/llm-provider/components/llm-models/EditLlmModelDrawer.tsx`

| line | state | condition |
|---|---|---|
| 94 | overlay | `title={isLocalModel ? 'Edit Local Model' : 'Edit Remote Model'}` |

### `modules/llm-provider/components/LlmModelsSection.tsx`

| line | state | condition |
|---|---|---|
| 301 | loading | `<Loading />` |

### `modules/llm-provider/components/LocalProviderSettings.tsx`

| line | state | condition |
|---|---|---|
| 35 | loading | `if (!currentProvider && (loading \|\| !isInitialized)) {` |

### `modules/mcp/chat-extension/extension.tsx`

| line | state | condition |
|---|---|---|
| 69 | error | `{(toolCall.status === 'completed' \|\| toolCall.status === 'error') && (` |
| 69 | error | `{(toolCall.status === 'completed' \|\| toolCall.status === 'error') && (` |
| 70 | error | `<span` |
| 132 | error | `{toolCall.error && (` |
| 133 | error | `<Alert` |
| 294 | error | `<CircleX className="text-destructive" />` |

### `modules/projects/chat-extension/extension.tsx`

| line | state | condition |
|---|---|---|
| 413 | loading | `if (state.kind === 'loading') {` |

### `modules/projects/components/ProjectFormDrawer.tsx`

| line | state | condition |
|---|---|---|
| 168 | overlay | `title={isEdit ? 'Edit Project' : 'New Project'}` |

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
| `modules/auth/Auth.store.ts` | 111 |
| `modules/chat/core/extensions/registry.tsx` | 111 |
| `modules/workflow/components/workflowElicitSchema.ts` | 108 |
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
| `modules/hardware/HardwareMonitor.tsx` | 61 |
| `modules/settings/SettingsPage.tsx` | 61 |
| `modules/layouts/app-layout/components/Drawer.tsx` | 60 |
