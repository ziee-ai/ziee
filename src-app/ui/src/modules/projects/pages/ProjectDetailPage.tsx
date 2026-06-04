import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useNavigate, useParams } from 'react-router-dom'
import { App, Button, Card, Divider, Flex, Popconfirm, Spin, Typography } from 'antd'
import {
  ArrowLeftOutlined,
  CloseCircleOutlined,
  CopyOutlined,
  DeleteOutlined,
  EditOutlined,
} from '@ant-design/icons'
import { Stores } from '@/core/stores'
import { Can, usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'
import { ProjectFormDrawer } from '@/modules/projects/components/ProjectFormDrawer'
import { ProjectKnowledgeSection } from '@/modules/projects/components/ProjectKnowledgeSection'
import { ProjectConversationsList } from '@/modules/projects/chat-extension/components/ProjectConversationsList'
import { ProjectDefaultsForm } from '@/modules/projects/components/ProjectDefaultsForm'
import { ProjectExtensionSlot } from '@/modules/projects/core/extensions'
import { HeaderBarContainer } from '@/modules/layouts/app-layout/components/HeaderBarContainer'
import { ProjectInlineChatInput } from '@/modules/projects/chat-extension/components/ProjectInlineChatInput'
import { useElementMinSize } from '@/modules/layouts/app-layout/hooks/useWindowMinSize'

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
 *   6. Advanced:     default assistant/model card, plus extension-
 *                    contributed panels (MCP defaults, future per-
 *                    project rate limits etc.) via the
 *                    `advanced_settings` slot.
 *
 * Previous design (round-3) buried conversations behind a Tabs component
 * with Knowledge / Conversations / MCP Settings as siblings, treating
 * them as equal-weight. They aren't — conversations are 80% of why
 * anyone visits this page. Round-4 redesign hoists them above the fold
 * with an inline chat input so users can start a new chat from this
 * page directly. The project chat extension's afterCreateConversation
 * hook files the freshly-created conversation into this project on
 * first send (assign endpoint), then the conversation.created event
 * subscriber below navigates to /projects/{id}/chat/{conv}.
 */
export function ProjectDetailPage() {
  const { projectId } = useParams<{ projectId: string }>()
  const navigate = useNavigate()
  const { message } = App.useApp()

  // Read all Stores fields at the top per [[project_stores_proxy_hooks]]
  // — the Stores proxy `get` trap calls useEffect + useStore (2 hooks per
  // access), so conditional reads cause "Rendered more hooks than during
  // the previous render".
  const { project, loading, error, conversations } = Stores.ProjectDetail
  const canDeleteConversations = usePermission(
    Permissions.ConversationsDelete,
  )

  // Bulk-selection state for the Conversations card. Lifted here so
  // the parent Card's `extra` slot can host the bulk-action toolbar
  // while the list itself just renders cards.
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set())
  const [bulkDeleting, setBulkDeleting] = useState(false)

  // Observe the page container width (not viewport — the same page
  // may render in a narrow desktop pane or full width depending on
  // sidebars / right panel). When the card itself is narrow the
  // bulk toolbar moves out of the Card title row into the body so
  // it doesn't overflow / wrap awkwardly.
  const pageContainerRef = useRef<HTMLDivElement>(null)
  const pageMinSize = useElementMinSize(pageContainerRef)
  const toolbarInCardBody = pageMinSize.sm

  // Drop selection on project switch so leftover ids from project A
  // can't trigger a bulk-delete after navigating to project B.
  useEffect(() => {
    setSelectedIds(new Set())
  }, [projectId])

  // Prune selectedIds against the currently-loaded conversations on
  // every list change (delete, attach, detach can mutate the array
  // mid-selection). A stable Set instance when nothing changed avoids
  // re-rendering the list with a "new" Set on every render.
  const visibleConversationIds = useMemo(
    () => new Set(conversations.map(c => c.id)),
    [conversations],
  )
  useEffect(() => {
    setSelectedIds(prev => {
      const next = new Set<string>()
      for (const id of prev) {
        if (visibleConversationIds.has(id)) next.add(id)
      }
      return next.size === prev.size ? prev : next
    })
  }, [visibleConversationIds])

  const handleToggleSelect = useCallback((id: string) => {
    setSelectedIds(prev => {
      const next = new Set(prev)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })
  }, [])

  const handleSelectAll = useCallback(() => {
    setSelectedIds(new Set(visibleConversationIds))
  }, [visibleConversationIds])

  const handleDeselectAll = useCallback(() => {
    setSelectedIds(new Set())
  }, [])

  const handleBulkDelete = useCallback(async () => {
    if (selectedIds.size === 0) return
    setBulkDeleting(true)
    const ids = Array.from(selectedIds)
    let succeeded = 0
    let failed = 0
    for (const id of ids) {
      try {
        await Stores.ChatHistory.__state.deleteConversation(id)
        succeeded += 1
      } catch {
        failed += 1
      }
    }
    setBulkDeleting(false)
    setSelectedIds(new Set())
    if (failed === 0) {
      message.success(
        `Deleted ${succeeded} conversation${succeeded === 1 ? '' : 's'}`,
      )
    } else {
      message.warning(
        `Deleted ${succeeded}, ${failed} failed`,
      )
    }
  }, [selectedIds, message])

  const handleBulkRemoveFromProject = useCallback(async () => {
    if (selectedIds.size === 0 || !projectId) return
    setBulkDeleting(true)
    const ids = Array.from(selectedIds)
    let succeeded = 0
    let failed = 0
    for (const id of ids) {
      try {
        await Stores.Projects.detachConversation(projectId, id)
        succeeded += 1
      } catch {
        failed += 1
      }
    }
    setBulkDeleting(false)
    setSelectedIds(new Set())
    if (failed === 0) {
      message.success(
        `Removed ${succeeded} conversation${succeeded === 1 ? '' : 's'} from project`,
      )
    } else {
      message.warning(
        `Removed ${succeeded}, ${failed} failed`,
      )
    }
  }, [selectedIds, projectId, message])

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

  // ChatInput integration. The chat module is project-unaware: it
  // creates an unfiled conversation on first send. The project
  // chat extension at `src-app/ui/src/modules/chat/extensions/
  // project/extension.tsx` runs `afterCreateConversation`, reads
  // `Stores.ProjectDetail.project`, and assigns the conversation
  // into this project via POST /projects/{id}/conversations/{conv}.
  // The post-hook conversation (with project_id populated) reaches
  // us via the `conversation.created` event, and we navigate to the
  // project-namespaced URL.
  useEffect(() => {
    if (!projectId) return
    // Clear stale chat state from a prior session so the next send
    // takes the auto-create branch (Stores.Chat.conversation === null).
    Stores.Chat.reset()

    const unsubscribe = Stores.EventBus.on(
      'conversation.created',
      event => {
        navigate(`/projects/${projectId}/chat/${event.data.conversation.id}`)
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
        <div
          ref={pageContainerRef}
          className="flex flex-col gap-3 max-w-4xl mx-auto p-4"
        >
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
            <ProjectInlineChatInput />
          </section>

          {/* 2. Recent conversations — full-width list, no tab. The
                second-most-important UI element after the chat input.
                Card wrapper matches Project details / Advanced so the
                three primary stacked blocks read as one visual rhythm.
                Bulk-action toolbar appears when ≥1 conversation is
                selected; placement (Card `extra` vs Card body) is
                width-responsive — narrow pages move it into the body
                so it doesn't overflow the title row. */}
          {(() => {
            const bulkToolbar = selectedIds.size > 0 ? (
              <Flex align="center" className="gap-2 flex-wrap">
                <Text strong>{selectedIds.size} selected</Text>
                <Button
                  type="text"
                  size="small"
                  icon={<CloseCircleOutlined />}
                  onClick={handleDeselectAll}
                >
                  Clear
                </Button>
                <Button
                  type="text"
                  size="small"
                  onClick={handleSelectAll}
                  disabled={
                    selectedIds.size === visibleConversationIds.size
                  }
                >
                  Select all
                </Button>
                <Popconfirm
                  title="Remove from project?"
                  description={`Detach ${selectedIds.size} conversation${selectedIds.size === 1 ? '' : 's'} from this project? They become unfiled (not deleted).`}
                  onConfirm={handleBulkRemoveFromProject}
                  okText="Remove"
                  cancelText="Cancel"
                  okButtonProps={{ loading: bulkDeleting }}
                >
                  <Button type="text" size="small" loading={bulkDeleting}>
                    Remove from project
                  </Button>
                </Popconfirm>
                {canDeleteConversations && (
                  <Popconfirm
                    title="Delete conversations?"
                    description={`Permanently delete ${selectedIds.size} conversation${selectedIds.size === 1 ? '' : 's'} and all messages.`}
                    onConfirm={handleBulkDelete}
                    okText="Delete"
                    cancelText="Cancel"
                    okType="danger"
                    okButtonProps={{ loading: bulkDeleting }}
                  >
                    <Button
                      type="text"
                      size="small"
                      danger
                      icon={<DeleteOutlined />}
                      loading={bulkDeleting}
                    >
                      Delete
                    </Button>
                  </Popconfirm>
                )}
              </Flex>
            ) : null

            return (
              <Card
                title="Conversations"
                data-test-section="conversations"
                extra={!toolbarInCardBody ? bulkToolbar : null}
              >
                {toolbarInCardBody && bulkToolbar && (
                  <div className="mb-3 flex justify-end">{bulkToolbar}</div>
                )}
                <ProjectConversationsList
                  projectId={project.id}
                  selectedIds={selectedIds}
                  onToggleSelect={handleToggleSelect}
                />
              </Card>
            )
          })()}

          {/* 3. Project metadata card — About, Instructions, Knowledge
                grouped in one Card in that order. About is the human
                summary; Instructions is the model-facing system prompt
                stacked into every conversation in the project; Knowledge
                is the attached files. Dividers between the three
                sub-sections match the peer settings-page convention
                (multiple related sections inside a single Card,
                separated by `Divider` rather than fragmenting into
                multiple cards). */}
          {/* The card-level Edit button opens the ProjectFormDrawer
              which edits ALL three subsections (About / Instructions /
              Knowledge live in the same form), so it makes more sense
              as a card-extra than a per-section button on Instructions
              alone. */}
          <Card
            title="Project details"
            data-test-section="project-meta"
            extra={
              <Can permission={Permissions.ProjectsEdit}>
                <Button
                  type="text"
                  icon={<EditOutlined />}
                  onClick={handleEdit}
                  aria-label="Edit project details"
                >
                  Edit
                </Button>
              </Can>
            }
          >
            <Flex vertical>
              <section data-test-section="description">
                <Text strong className="block mb-2">
                  About
                </Text>
                {project.description ? (
                  <Paragraph
                    type="secondary"
                    className="!mb-0 whitespace-pre-wrap"
                    data-test-description={project.description}
                  >
                    {project.description}
                  </Paragraph>
                ) : (
                  <Text type="secondary" className="italic">
                    No description yet — click Edit to add one.
                  </Text>
                )}
              </section>

              <Divider className="!my-2" />

              <section data-test-section="instructions">
                <Text strong className="block mb-2">
                  Instructions
                </Text>
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

              <Divider className="!my-2" />

              <section data-test-section="knowledge">
                <ProjectKnowledgeSection />
              </section>
            </Flex>
          </Card>

          {/* 4. Advanced — default assistant/model in its own card,
                followed by extension-contributed panels (MCP defaults,
                future per-project rate limits etc.) via the
                advanced_settings slot. The MCP panel ships its own
                Configure button + modal. */}
          <Card title="Advanced" data-test-section="advanced">
            {/* Default assistant + default model — inline auto-save
                selects (one PATCH per change). These used to live in
                the ProjectFormDrawer with the content fields, but
                they're configuration shape (pick a foreign key that
                snapshots into new conversations), not content; inline
                editing is the right ergonomic. See ProjectDefaultsForm
                for the tri-state-null reasoning. */}
            <ProjectDefaultsForm project={project} />
          </Card>

          <ProjectExtensionSlot name="advanced_settings" />
        </div>
      </div>

      <ProjectFormDrawer />
    </div>
  )
}
