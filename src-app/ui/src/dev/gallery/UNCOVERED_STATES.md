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
- **138** STATE gaps not allow-listed — the actionable queue.
- 9044 generic branch forks not allow-listed (informational).

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

### `modules/auth/AuthGuard.tsx`

| line | state | condition |
|---|---|---|
| 47 | loading | `if (isInitializing \|\| needsSetup === null) {` |
| 47 | loading | `if (isInitializing \|\| needsSetup === null) {` |
| 47 | loading | `if (isInitializing \|\| needsSetup === null) {` |

### `modules/auth/LoginForm.tsx`

| line | state | condition |
|---|---|---|
| 52 | error | `{error && (` |
| 53 | error | `<div className="py-4" role="alert" aria-live="assertive">` |

### `modules/auth/ProviderButtons.tsx`

| line | state | condition |
|---|---|---|
| 43 | loading | `if (isLoading \|\| !hasLoaded) {` |
| 43 | loading | `if (isLoading \|\| !hasLoaded) {` |
| 43 | loading | `if (isLoading \|\| !hasLoaded) {` |
| 51 | error | `if (error) {` |
| 63 | empty | `if (!providers \|\| providers.length === 0) return null` |
| 63 | empty | `if (!providers \|\| providers.length === 0) return null` |
| 63 | empty | `if (!providers \|\| providers.length === 0) return null` |

### `modules/auth/RegisterForm.tsx`

| line | state | condition |
|---|---|---|
| 89 | error | `{error && (` |
| 90 | error | `<Alert` |

### `modules/chat/components/ChatMessage.tsx`

| line | state | condition |
|---|---|---|
| 18 | empty | `if (!message.contents \|\| message.contents.length === 0) {` |

### `modules/chat/components/MessageList.tsx`

| line | state | condition |
|---|---|---|
| 19 | loading | `if (!loading && messagesArray.length === 0) {` |

### `modules/chat/core/utils/useStreamdownComponents.tsx`

| line | state | condition |
|---|---|---|
| 144 | empty | `if (src.startsWith('/')) return <img {...props} />` |

### `modules/chat/pages/ChatHistoryPage.tsx`

| line | state | condition |
|---|---|---|
| 143 | error | `<div className={cn('flex flex-1 flex-col w-full', nativeScroll ? '' : 'overflow-hidden')}>` |

### `modules/chat/pages/ConversationPage.tsx`

| line | state | condition |
|---|---|---|
| 101 | loading | `if (loading && !conversation) {` |
| 101 | loading | `if (loading && !conversation) {` |
| 108 | loading | `if (!loading && !conversation) {` |
| 142 | error | `<div className="w-full max-w-4xl mx-auto px-4 pt-4">` |

### `modules/citations/project-extension/components/ProjectBibliographyInlinePreview.tsx`

| line | state | condition |
|---|---|---|
| 55 | empty | `<Button` |

### `modules/citations/project-extension/components/ProjectBibliographyManagePanel.tsx`

| line | state | condition |
|---|---|---|
| 74 | loading | `<Spin label="Loading" />` |
| 75 | empty | `) : entries.length === 0 ? (` |
| 76 | empty | `<Empty description="No references in this project yet." data-testid="cite-bib-panel-empty" />` |

### `modules/file-rag/components/sections/ChunkingSection.tsx`

| line | state | condition |
|---|---|---|
| 128 | error | `<Alert data-testid="filerag-chunking-error-alert" tone="error" className="!mb-4" title={error \|\| validationError} />` |

### `modules/file/components/FileCard.tsx`

| line | state | condition |
|---|---|---|
| 136 | error | `<Text className="!text-[9px] rounded px-1 text-white bg-destructive">` |
| 160 | error | `{isError && onRetry ? (` |
| 160 | error | `{isError && onRetry ? (` |
| 161 | error | `<AttachmentActions>` |
| 205 | error | `onRetry ? (` |

### `modules/file/project-extension/components/ProjectFilesInlinePreview.tsx`

| line | state | condition |
|---|---|---|
| 25 | empty | `{filesLoading && files.length === 0 ? (` |
| 25 | empty | `{filesLoading && files.length === 0 ? (` |
| 26 | empty | `<div className="flex justify-center py-4">` |
| 29 | empty | `) : files.length === 0 ? (` |
| 30 | empty | `<Button` |

### `modules/file/project-extension/components/ProjectFilesManagePanel.tsx`

| line | state | condition |
|---|---|---|
| 256 | empty | `uploadingRows.length === 0 ? null : (` |
| 257 | empty | `<div className="flex flex-col mb-3 gap-2">` |
| 282 | empty | `) : files.length === 0 ? (` |
| 283 | empty | `<Empty` |

### `modules/file/viewers/pdf/body.tsx`

| line | state | condition |
|---|---|---|
| 44 | empty | `if (!root \|\| file.preview_page_count === 0) return` |
| 71 | empty | `if (file.preview_page_count === 0) {` |

### `modules/file/viewers/shared/chrome.tsx`

| line | state | condition |
|---|---|---|
| 30 | empty | `const mode = Stores.File.fileViewModes.get(file.id) ?? 'compiled'` |
| 30 | empty | `const mode = Stores.File.fileViewModes.get(file.id) ?? 'compiled'` |

### `modules/file/viewers/tabular/XlsxBody.tsx`

| line | state | condition |
|---|---|---|
| 88 | error | `if (loadError) {` |
| 97 | loading | `if (!fileBinaryContent \|\| loading) {` |
| 97 | loading | `if (!fileBinaryContent \|\| loading) {` |
| 97 | loading | `if (!fileBinaryContent \|\| loading) {` |
| 101 | empty | `if (sheets.length === 0) {` |

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

### `modules/literature/components/LiteratureToolResultCard.tsx`

| line | state | condition |
|---|---|---|
| 68 | empty | `<Text type="secondary" className="text-xs block mb-2" data-testid="lit-tool-result-empty">` |

### `modules/llm-local-runtime/components/AvailableVersionsCard.tsx`

| line | state | condition |
|---|---|---|
| 162 | empty | `) : readyUpstream.length === 0 ? (` |
| 163 | empty | `<Text type="secondary">` |
| 300 | error | `{progress && <DownloadProgressLine progress={progress} />}` |
| 300 | error | `{progress && <DownloadProgressLine progress={progress} />}` |
| 301 | error | `{failed && progress?.error && (` |
| 301 | error | `{failed && progress?.error && (` |
| 302 | error | `<Text type="secondary">{progress.error}</Text>` |

### `modules/llm-local-runtime/components/VersionModelsBlock.tsx`

| line | state | condition |
|---|---|---|
| 80 | empty | `<Empty` |

### `modules/llm-provider/components/downloads/DownloadsSection.tsx`

| line | state | condition |
|---|---|---|
| 21 | empty | `if (providerDownloads.length === 0) {` |

### `modules/llm-provider/components/GroupLlmProvidersAssignmentDrawer.tsx`

| line | state | condition |
|---|---|---|
| 73 | overlay | `title={`Assign LLM Providers - ${selectedGroup?.name \|\| ''}`}` |

### `modules/llm-provider/components/llm-models/AddLocalLlmModelDownloadDrawer.tsx`

| line | state | condition |
|---|---|---|
| 159 | empty | `if (res.files.length === 0) {` |
| 359 | overlay | `title={viewMode ? 'View Download Details' : 'Download from Repository'}` |
| 359 | overlay | `title={viewMode ? 'View Download Details' : 'Download from Repository'}` |
| 368 | loading | `canCancelDownload &&` |
| 369 | loading | `viewDownload &&` |
| 415 | error | `<Card title="Download Progress" className="mb-4" data-testid="llm-download-progress-card">` |
| 416 | error | `{viewDownload.status === 'failed' && viewDownload.error_message ? (` |
| 416 | error | `{viewDownload.status === 'failed' && viewDownload.error_message ? (` |
| 417 | error | `<Text type="danger">{viewDownload.error_message}</Text>` |

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
| 35 | loading | `if (!currentProvider && (loading \|\| !isInitialized)) {` |
| 35 | loading | `if (!currentProvider && (loading \|\| !isInitialized)) {` |
| 35 | loading | `if (!currentProvider && (loading \|\| !isInitialized)) {` |

### `modules/llm-provider/widgets/LLMProviderGroupWidget.tsx`

| line | state | condition |
|---|---|---|
| 57 | error | `<Text type="danger" className="text-xs">` |

### `modules/mcp/chat-extension/extension.tsx`

| line | state | condition |
|---|---|---|
| 123 | error | `{toolCall.result !== undefined && (` |

### `modules/mcp/components/common/KeyValueSecretEditor.tsx`

| line | state | condition |
|---|---|---|
| 172 | empty | `{fields.length === 0 && (` |
| 173 | empty | `<Text type="secondary" className="text-xs">` |

### `modules/mcp/components/system/GroupSystemMcpServersAssignmentDrawer.tsx`

| line | state | condition |
|---|---|---|
| 76 | overlay | `title={`Assign System MCP Servers - ${selectedGroup?.name \|\| ''}`}` |

### `modules/mcp/components/system/McpUserPolicyCard.tsx`

| line | state | condition |
|---|---|---|
| 149 | empty | `{noTransports && (` |
| 150 | empty | `<Alert` |

### `modules/mcp/project-extension/components/ProjectMcpSettingsPanel.tsx`

| line | state | condition |
|---|---|---|
| 133 | loading | `{loading && !settings ? (` |
| 133 | loading | `{loading && !settings ? (` |
| 134 | loading | `<Skeleton />` |
| 167 | empty | `{noRules && (` |
| 168 | empty | `<Empty` |

### `modules/mcp/widgets/GroupSystemMcpServersWidget.tsx`

| line | state | condition |
|---|---|---|
| 67 | error | `<Text type="danger" className="text-xs">` |

### `modules/memory/components/CoreMemoryBlocksEditor.tsx`

| line | state | condition |
|---|---|---|
| 215 | overlay | `data-testid={existing ? 'memory-core-block-edit-dialog' : 'memory-core-block-create-dialog'}` |

### `modules/onboarding/OnboardingRedirect.tsx`

| line | state | condition |
|---|---|---|
| 45 | loading | `if (isInitializing) return` |
| 46 | loading | `if (!isAuthenticated \|\| !user) return` |
| 46 | loading | `if (!isAuthenticated \|\| !user) return` |
| 46 | loading | `if (!isAuthenticated \|\| !user) return` |

### `modules/projects/chat-extension/extension.tsx`

| line | state | condition |
|---|---|---|
| 413 | loading | `if (state.kind === 'loading') {` |

### `modules/projects/components/AddToProjectModal.tsx`

| line | state | condition |
|---|---|---|
| 108 | error | `{error && (` |
| 109 | error | `<Text type="danger" className="text-sm">` |

### `modules/projects/components/ProjectFormDrawer.tsx`

| line | state | condition |
|---|---|---|
| 129 | loading | `if (loading) return` |
| 168 | overlay | `title={isEdit ? 'Edit Project' : 'New Project'}` |

### `modules/summarization/chat-extension/components/SummarizationStatusPill.tsx`

| line | state | condition |
|---|---|---|
| 145 | loading | `<Loader2 className="animate-spin" />` |

### `modules/user-profile/UserProfileWidget.tsx`

| line | state | condition |
|---|---|---|
| 86 | loading | `if (isInitializing \|\| isLoading) {` |
| 86 | loading | `if (isInitializing \|\| isLoading) {` |
| 86 | loading | `if (isInitializing \|\| isLoading) {` |

### `modules/workflow/components/DryRunPreviewDialog.tsx`

| line | state | condition |
|---|---|---|
| 58 | loading | `{loading && <Spin data-testid="wf-dry-run-spin" label="Running dry run" className="block my-8 mx-auto" />}` |
| 58 | loading | `{loading && <Spin data-testid="wf-dry-run-spin" label="Running dry run" className="block my-8 mx-auto" />}` |
| 59 | error | `{error && <Alert data-testid="wf-dry-run-error-alert" tone="error" title={error} />}` |
| 59 | error | `{error && <Alert data-testid="wf-dry-run-error-alert" tone="error" title={error} />}` |
| 60 | error | `{result && (` |

### `modules/workflow/components/EditableArrayTable.tsx`

| line | state | condition |
|---|---|---|
| 358 | empty | `<tr>` |

### `modules/workflow/components/StepArtifacts.tsx`

| line | state | condition |
|---|---|---|
| 28 | empty | `if (artifacts.length === 0) return null` |

### `modules/workflow/components/WorkflowElicitForm.tsx`

| line | state | condition |
|---|---|---|
| 484 | error | `{error && (` |
| 485 | error | `<Alert data-testid="wf-elicit-error-alert" tone="error" title={error} />` |

### `modules/workflow/components/WorkflowRunProgressView.tsx`

| line | state | condition |
|---|---|---|
| 178 | error | `{run.error && <Alert data-testid="wf-progress-error-alert" tone="error" title={run.error} />}` |
| 178 | error | `{run.error && <Alert data-testid="wf-progress-error-alert" tone="error" title={run.error} />}` |
| 240 | error | `{s.error && (` |
| 241 | error | `<Text type="danger" className="text-xs">` |
| 268 | error | `{(s.status === 'completed' \|\| s.status === 'failed') && (` |
| 268 | error | `{(s.status === 'completed' \|\| s.status === 'failed') && (` |
| 269 | error | `<Space direction="horizontal" size={4} wrap>` |
| 307 | empty | `{steps.length === 0 && !terminal && (` |
| 307 | empty | `{steps.length === 0 && !terminal && (` |
| 308 | empty | `<Text type="secondary" className="text-xs">` |

### `modules/workflow/components/WorkflowTestsPanel.tsx`

| line | state | condition |
|---|---|---|
| 60 | loading | `{loading && <Spin label="Loading" />}` |
| 60 | loading | `{loading && <Spin label="Loading" />}` |
| 61 | error | `{error && <Alert data-testid="wf-tests-error-alert" tone="error" title={error} />}` |
| 61 | error | `{error && <Alert data-testid="wf-tests-error-alert" tone="error" title={error} />}` |
| 62 | error | `{result && (` |
| 66 | error | `{result.failed > 0 && <Tag variant="outline" data-testid="wf-tests-failed-tag" tone="error">{result.failed} failed</Tag>}` |
| 66 | error | `{result.failed > 0 && <Tag variant="outline" data-testid="wf-tests-failed-tag" tone="error">{result.failed} failed</Tag>}` |
| 67 | error | `{result.skipped > 0 && (` |

## Generic branch forks (informational — top files by count)

| file | uncovered forks |
|---|---|
| `modules/mcp/components/common/McpServerDrawer.tsx` | 395 |
| `modules/chat/core/stores/Chat.store.ts` | 221 |
| `modules/mcp/stores/McpComposer.store.ts` | 206 |
| `modules/mcp/chat-extension/extension.tsx` | 156 |
| `modules/llm-provider/components/llm-models/AddLocalLlmModelDownloadDrawer.tsx` | 152 |
| `modules/layouts/app-layout/AppLayout.tsx` | 142 |
| `modules/workflow/components/WorkflowElicitForm.tsx` | 141 |
| `modules/workflow/components/EditableArrayTable.tsx` | 123 |
| `modules/file/stores/File.store.ts` | 115 |
| `modules/auth/Auth.store.ts` | 114 |
| `modules/hub/modules/llm-models/components/ModelHubCard.tsx` | 113 |
| `modules/chat/core/extensions/registry.tsx` | 111 |
| `modules/workflow/components/workflowElicitSchema.ts` | 110 |
| `components/ui/kit/multi-select.tsx` | 106 |
| `modules/llm-repository/components/LlmRepositoryDrawer.tsx` | 100 |
| `modules/hardware/HardwareSettings.tsx` | 99 |
| `modules/code-sandbox/components/_rootfsShared.tsx` | 97 |
| `modules/workflow/components/WorkflowRunProgressView.tsx` | 93 |
| `modules/auth-providers/components/AuthProviderEditDrawer.tsx` | 91 |
| `modules/file/project-extension/components/ProjectFilesManagePanel.tsx` | 89 |
| `components/ui/kit/tree.tsx` | 86 |
| `modules/layouts/app-layout/components/LeftSidebar.tsx` | 86 |
| `modules/projects/chat-extension/extension.tsx` | 84 |
| `modules/llm-provider/components/llm-models/AddLocalLlmModelUploadDrawer.tsx` | 83 |
| `modules/workflow/stores/WorkflowRun.store.ts` | 82 |
| `modules/mcp/components/McpConfigModal.tsx` | 80 |
| `modules/mcp/chat-extension/components/ElicitationFormContent.tsx` | 75 |
| `modules/llm-provider/components/LlmModelsSection.tsx` | 74 |
| `modules/llm-local-runtime/components/AvailableVersionsCard.tsx` | 66 |
| `modules/file/components/FileCard.tsx` | 65 |
