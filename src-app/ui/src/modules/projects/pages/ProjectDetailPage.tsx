import { useEffect } from 'react'
import { useNavigate, useParams } from 'react-router-dom'
import { App, Button, Spin, Typography } from 'antd'
import {
  ArrowLeftOutlined,
  CopyOutlined,
  EditOutlined,
  ToolOutlined,
} from '@ant-design/icons'
import { Stores } from '@/core/stores'
import { Can } from '@/core/permissions'
import { Permissions } from '@/api-client/types'
import { ProjectFormDrawer } from '@/modules/projects/components/ProjectFormDrawer'
import { ProjectFilesManageDrawer } from '@/modules/projects/components/ProjectFilesManageDrawer'
import { ProjectConversationsList } from '@/modules/projects/components/ProjectConversationsList'
import { HeaderBarContainer } from '@/modules/layouts/app-layout/components/HeaderBarContainer'
import { ChatInput } from '@/modules/chat/components/ChatInput'
import { McpConfigModal } from '@/modules/chat/extensions/mcp/components/McpConfigModal'

const { Title, Text, Paragraph } = Typography

/**
 * Project detail page — Option A layout.
 *
 * Top to bottom:
 *   1. Header bar:   back, project name, Edit, Duplicate
 *   2. ChatInput:    inline composer that starts a new conversation in
 *                    this project on send (latch project_id, listen for
 *                    conversation.created → navigate /chat/{id})
 *   3. Conversations:full-width list (the primary thing users came for —
 *                    NOT hidden in a tab)
 *   4. Knowledge:    compact chip preview + Manage drawer
 *   5. Instructions: inline preview + Edit affordance
 *   6. Advanced:     defaults summary + "Configure MCP defaults" button
 *                    (project-scope MCP; opens the shared McpConfigModal)
 *
 * Previous design (round-3) buried conversations behind a Tabs component
 * with Knowledge / Conversations / MCP Settings as siblings, treating
 * them as equal-weight. They aren't — conversations are 80% of why
 * anyone visits this page. Round-4 redesign hoists them above the fold
 * with an inline chat input so users can start a new chat from this
 * page directly instead of navigating to /chat?project_id=… first.
 */
export function ProjectDetailPage() {
  const { projectId } = useParams<{ projectId: string }>()
  const navigate = useNavigate()
  const { message } = App.useApp()

  // Read all Stores fields at the top per [[project_stores_proxy_hooks]]
  // — the Stores proxy `get` trap calls useEffect + useStore (2 hooks per
  // access), so conditional reads cause "Rendered more hooks than during
  // the previous render".
  const { project, loading, error } = Stores.ProjectDetail

  useEffect(() => {
    if (projectId) {
      void Stores.ProjectDetail.loadProject(projectId)
    }
  }, [projectId])

  useEffect(() => {
    if (error) {
      message.error(error)
      Stores.ProjectDetail.clearProjectDetailError()
    }
  }, [error, message])

  // ChatInput integration. The ChatInput component is self-contained:
  // it calls Stores.Chat.sendMessage() which auto-creates a conversation
  // if `Stores.Chat.conversation` is null. By latching the project's id
  // into Stores.Chat.pendingProjectId BEFORE the send, the new
  // conversation gets created with project_id set on the backend (which
  // also triggers the MCP-settings snapshot).
  //
  // After creation, the chat store emits a `conversation.created` event
  // that we listen for here to navigate to /chat/{id} — same pattern as
  // NewChatPage.
  useEffect(() => {
    if (!projectId) return
    // Clear any stale chat state from a prior session and latch the
    // project id so the next send creates a conversation INSIDE this
    // project. Pre-latching once on mount is enough; the chat store
    // consumes-and-clears the value during conversation creation.
    Stores.Chat.reset()
    Stores.Chat.setPendingProjectId(projectId)

    const unsubscribe = Stores.EventBus.on(
      'conversation.created',
      event => {
        navigate(`/chat/${event.data.conversation.id}`)
      },
      'ProjectDetailPage',
    )
    return () => {
      unsubscribe()
    }
  }, [projectId, navigate])

  if (!projectId) {
    return null
  }

  if (loading || !project) {
    return (
      <div className="h-full flex items-center justify-center">
        <Spin />
      </div>
    )
  }

  const handleEdit = () => Stores.ProjectDrawer.openProjectDrawer(project)

  const handleDuplicate = async () => {
    try {
      const copy = await Stores.Projects.duplicateProject(project.id)
      // `undefined` = a prior duplicate is still in flight; bail
      // silently so the user doesn't get a misleading toast or get
      // navigated to a phantom project page.
      if (!copy) return
      message.success(`Duplicated as "${copy.name}"`)
      navigate(`/projects/${copy.id}`)
    } catch (_err) {
      message.error('Failed to duplicate project')
    }
  }

  const handleConfigureMcp = () => {
    Stores.Chat.McpStore.openConfigModalForProject(project)
  }

  // MCP summary used in the Advanced section.
  const approvalMode = project.mcp_approval_mode || 'manual_approve'
  const approvalLabel =
    approvalMode === 'auto_approve'
      ? 'Auto approve'
      : approvalMode === 'disabled'
      ? 'Disabled'
      : 'Manual approve'
  const autoApprovedCount = Array.isArray(project.mcp_auto_approved_tools)
    ? (project.mcp_auto_approved_tools as unknown[]).length
    : 0
  const disabledCount = Array.isArray(project.mcp_disabled_servers)
    ? (project.mcp_disabled_servers as unknown[]).length
    : 0

  return (
    <div className="h-full flex flex-col overflow-hidden">
      <HeaderBarContainer>
        <div className="h-full flex items-center justify-between w-full gap-2">
          <div className="flex items-center min-w-0 gap-2">
            <Button
              type="text"
              icon={<ArrowLeftOutlined />}
              onClick={() => navigate('/projects')}
              aria-label="Back to projects"
            />
            <Title
              level={4}
              className="!m-0 !leading-tight truncate"
              data-test-project-title={project.name}
            >
              {project.name}
            </Title>
          </div>
          <div className="flex items-center gap-1">
            <Can permission={Permissions.ProjectsEdit}>
              <Button type="text" icon={<EditOutlined />} onClick={handleEdit}>
                Edit
              </Button>
            </Can>
            {/* Duplicate is gated by BOTH create + read on the
                backend (handlers.rs: RequirePermissions<(ProjectsCreate,
                ProjectsRead)>). Match that on the FE so the button
                only renders when the user can actually succeed
                (audit Q12). */}
            <Can
              permission={{
                allOf: [
                  Permissions.ProjectsCreate,
                  Permissions.ProjectsRead,
                ],
              }}
            >
              <Button
                type="text"
                icon={<CopyOutlined />}
                onClick={handleDuplicate}
              >
                Duplicate
              </Button>
            </Can>
          </div>
        </div>
      </HeaderBarContainer>

      <div className="flex-1 overflow-y-auto">
        <div className="max-w-4xl mx-auto p-4 space-y-6">
          {/* 1. Inline chat input — start a new conversation in this project.
                The label above the input is intentional: users land here
                from the sidebar nav, expecting to either resume an existing
                chat or start a new one. The input is the primary CTA. */}
          <section
            aria-label="Start a new conversation in this project"
            data-test-section="chat-input"
          >
            <Text type="secondary" className="block mb-2 text-sm">
              Start a new conversation in this project
            </Text>
            <ChatInput />
          </section>

          {/* 2. Recent conversations — full-width list, no tab. The
                second-most-important UI element after the chat input. */}
          <section data-test-section="conversations">
            <div className="flex items-center justify-between mb-2">
              <Text strong>Conversations</Text>
            </div>
            <ProjectConversationsList projectId={project.id} />
          </section>

          {/* 3. Project knowledge — compact inline preview with a
                "Manage" button that opens the full file manager in a
                drawer. Stays visible (not behind a tab) but doesn't
                dominate the page. */}
          <section data-test-section="knowledge">
            <div className="flex items-center justify-between mb-2">
              <Text strong>Project knowledge</Text>
            </div>
            <ProjectFilesManageDrawer projectId={project.id} />
          </section>

          {/* 4. Instructions — inline preview + Edit. Important enough to
                keep visible (it shapes every conversation in the project)
                but not bigger than conversations. */}
          <section data-test-section="instructions">
            <div className="flex items-center justify-between mb-2">
              <Text strong>Instructions</Text>
              <Can permission={Permissions.ProjectsEdit}>
                <Button
                  type="text"
                  size="small"
                  icon={<EditOutlined />}
                  onClick={handleEdit}
                  aria-label="Edit instructions"
                >
                  Edit
                </Button>
              </Can>
            </div>
            {project.instructions ? (
              <Paragraph
                className="whitespace-pre-wrap !mb-0"
                data-test-instructions={project.instructions}
              >
                {project.instructions}
              </Paragraph>
            ) : (
              <Text type="secondary" className="italic">
                No instructions yet — click Edit to add some.
              </Text>
            )}
          </section>

          {/* 5. Description (if set) — small inline block, lowest
                priority. Treated as separate from instructions because
                "description" is human-readable summary; "instructions"
                is the model-facing system prompt. */}
          {project.description && (
            <section data-test-section="description">
              <Text strong className="block mb-2">
                About
              </Text>
              <Paragraph type="secondary" className="!mb-0">
                {project.description}
              </Paragraph>
            </section>
          )}

          {/* 6. Advanced — defaults summary + MCP defaults button.
                Demoted from a top-level tab to a stacked summary block.
                Each row is read-only; editing happens via Edit (drawer)
                or the MCP modal. */}
          <section data-test-section="advanced">
            <Text strong className="block mb-2">
              Advanced
            </Text>
            <div className="flex flex-col gap-2 text-sm">
              <div className="flex items-center justify-between">
                <Text type="secondary">Default assistant:</Text>
                <Text data-test-default-assistant-set={
                  project.default_assistant_id ? 'true' : 'false'
                }>
                  {project.default_assistant_id ? 'Set' : 'Not set'}
                </Text>
              </div>
              <div className="flex items-center justify-between">
                <Text type="secondary">Default model:</Text>
                <Text data-test-default-model-set={
                  project.default_model_id ? 'true' : 'false'
                }>
                  {project.default_model_id ? 'Set' : 'Not set'}
                </Text>
              </div>
              <div className="flex items-center justify-between">
                <Text type="secondary">MCP approval mode:</Text>
                <Text data-test-mcp-approval-mode={approvalMode}>
                  {approvalLabel}
                </Text>
              </div>
              <div className="flex items-center justify-between">
                <Text type="secondary">MCP auto-approved rules:</Text>
                <Text>{autoApprovedCount}</Text>
              </div>
              <div className="flex items-center justify-between">
                <Text type="secondary">MCP disabled rules:</Text>
                <Text>{disabledCount}</Text>
              </div>
              <Can permission={Permissions.ProjectsEdit}>
                <Button
                  icon={<ToolOutlined />}
                  onClick={handleConfigureMcp}
                  className="self-start mt-1"
                  aria-label="Configure MCP defaults"
                >
                  Configure MCP defaults
                </Button>
              </Can>
            </div>
          </section>
        </div>
      </div>

      <ProjectFormDrawer />
      {/* Shared MCP modal — controlled by Stores.Chat.McpStore. The
          Advanced "Configure MCP defaults" button above opens this in
          project scope; the dispatch rule
          (currentProjectId && !currentConversationId) routes saves to
          /projects/{id}/mcp-settings. */}
      <McpConfigModal />
    </div>
  )
}
