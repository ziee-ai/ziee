import { FolderOpen, CircleMinus, CirclePlus } from 'lucide-react'
import { useEffect, useState } from 'react'
import { Button, Confirm, Spin, Tag, Tooltip, message, dialog } from '@ziee/kit'
import type { DropdownItem } from '@ziee/kit'
import type { Conversation } from '@/api-client/types'
import { useNavigate } from 'react-router-dom'
import { ApiClient } from '@/api-client'
import { Permissions } from '@/api-client/types'
import type { Project } from '@/api-client/types'
import { hasPermissionNow, usePermission } from '@/core/permissions'
import { Stores } from '@/core/stores'
import {
  createExtension,
  type ChatExtension,
} from '@/modules/chat/core/extensions'
import { AddToProjectModal } from '@/modules/projects/components/AddToProjectModal'

/**
 * Frontend bridge between chat (project-unaware) and the projects
 * module. This file is the ONLY place on the chat side that touches
 * the projects module — chat's core code has zero project imports;
 * the projects module never reaches into chat's stores.
 *
 * Responsibilities:
 *   - `afterCreateConversation`: attach a freshly-created chat into
 *     the active project (via Projects.store).
 *   - `onConversationLoad`: resolve the conversation's project and
 *     cache it so the synchronous URL hooks can read it without a
 *     network call.
 *   - `conversationHref` / `conversationBackHref`: namespaced URL +
 *     back-button routing for project-bound conversations.
 *   - `renderConversationCardTrailing`: lazy per-card hover badge —
 *     "In project: X" when attached, "Add to project" otherwise.
 */

// Cache keyed by conversation id. Stores the full Project so the
// trailing component doesn't have to re-fetch by id.
//   * `undefined` (key absent) — never looked up
//   * `null` — looked up, no project (unfiled)
//   * Project — looked up, in this project
const conversationProjectCache = new Map<string, Project | null>()

// In-flight fetch promises keyed by conversation id. Dedup guard
// against:
//   1. React 18 StrictMode dev double-mount (each mount fires the
//      useEffect → would fire two network requests per real hover
//      without this).
//   2. Fast hover-off / hover-on cycles where the second mount
//      lands before the first request resolves.
//   3. Multiple ConversationCards somehow rendering trailing for
//      the same conversation simultaneously.
const inflightProjectLookups = new Map<string, Promise<Project | null>>()

function getCached(id: string): Project | null | undefined {
  return conversationProjectCache.get(id)
}

function setCached(id: string, value: Project | null) {
  conversationProjectCache.set(id, value)
}

/**
 * Cache-aware, in-flight-deduped lookup. Returns the same promise
 * to concurrent callers; populates the cache on resolve.
 * `forceRefresh` bypasses the cache (used by attach/detach event
 * handlers that need the latest server state).
 */
function loadProjectForConversation(
  conversationId: string,
  forceRefresh = false,
): Promise<Project | null> {
  // `GET /api/projects/by-conversation/{id}` requires projects::read, which
  // is granted to Administrators only (Chat Projects is opt-in per deployment,
  // migration 54) — NOT the default Users group. This lookup runs on EVERY
  // conversation load, so without this gate every non-projects chat user fired
  // a 403 on each open (swallowed by the catch below, but still a failed
  // request the runtime-health gate flags). A user without projects::read has
  // no projects, so the answer is always "unfiled" → cache null, skip the call.
  if (!hasPermissionNow(Permissions.ProjectsRead)) {
    setCached(conversationId, null)
    return Promise.resolve(null)
  }
  if (!forceRefresh) {
    const cached = getCached(conversationId)
    if (cached !== undefined) return Promise.resolve(cached)
    const inflight = inflightProjectLookups.get(conversationId)
    if (inflight) return inflight
  } else {
    // Force-refresh: drop cached value first so re-callers see the
    // in-flight promise instead of the stale value.
    conversationProjectCache.delete(conversationId)
  }

  const promise = ApiClient.Project.forConversation({
    conversation_id: conversationId,
  })
    .then(project => {
      // Backend returns Option<Project> — `null` means unfiled. Always
      // a 200; no catch needed for the normal "no project" case.
      const value = project ?? null
      setCached(conversationId, value)
      return value
    })
    .catch(() => {
      // Real network / auth errors only — treat as null so the trailing
      // falls back to "Add to project" rather than spinning forever.
      setCached(conversationId, null)
      return null
    })
    .finally(() => {
      inflightProjectLookups.delete(conversationId)
    })

  inflightProjectLookups.set(conversationId, promise)
  return promise
}

// Project UUID pattern at the start of the current path. Matches
// both `/projects/{id}` and `/projects/{id}/chat/{conv}`.
const PROJECT_URL_RE = /^\/projects\/([0-9a-f-]{36})(?:\/|$)/i

function projectIdFromUrl(): string | null {
  const m = window.location.pathname.match(PROJECT_URL_RE)
  return m ? m[1] : null
}


const projectExtension: ChatExtension = createExtension({
  name: 'project',
  description: 'Project bridge for chat (attach, URL routing, trailing).',
  priority: 10,

  onConversationLoad: async (conversation) => {
    // Force-refresh: loading a conversation is a strong signal that
    // any cached membership from a previous session is stale.
    const project = await loadProjectForConversation(conversation.id, true)
    if (project?.default_assistant_id) {
      // Seed the assistant picker with the project's default when
      // the user hasn't picked one. One-shot — won't override an
      // explicit user choice.
      const picker = Stores.AssistantPicker
      if (!picker.selectedAssistantId) {
        picker.selectAssistant(project.default_assistant_id)
      }
    }
  },

  afterCreateConversation: async (conversation) => {
    const projectId = projectIdFromUrl()
    if (!projectId) return
    try {
      const response = await Stores.Projects.attachConversation(
        projectId,
        conversation.id,
      )
      // Best-effort cache: we only have the project id here, not
      // the full Project. Use a stub so URL hooks work immediately;
      // a subsequent `onConversationLoad` fills in the full row.
      setCached(conversation.id, {
        id: projectId,
        name: '',
      } as Project)
      const { message_count: _mc, ...next } = response
      return next
    } catch (err) {
      console.error(
        '[project extension] attach failed; conversation stays unfiled.',
        err,
      )
      message.error('Failed to file this conversation into the project — saved as unfiled.')
      setCached(conversation.id, null)
      return
    }
  },

  conversationHref: (conversation) => {
    const cached = getCached(conversation.id)
    return cached ? `/projects/${cached.id}/chat/${conversation.id}` : undefined
  },

  conversationBackHref: (conversation) => {
    const cached = getCached(conversation.id)
    return cached ? `/projects/${cached.id}` : undefined
  },

  renderConversationCardTrailing: (conversation) => (
    <ProjectMembershipTrailing conversationId={conversation.id} />
  ),

  // Dropdown contributions for the sidebar's RecentConversationsWidget
  // (and any future conversation menu). Provides:
  //   - In-project conv: "Open project: NAME" + "Remove from project"
  //     (with popconfirm)
  //   - Unfiled conv: "Add to project" (opens AddToProjectModal)
  //
  // Implemented as a React hook (not a plain function) so the
  // overlay JSX can hold its own state (popconfirm/modal open
  // flags) alongside the menu items.
  useConversationMenu: (conversation) => {
    return useProjectMenuContribution(conversation)
  },
})

/**
 * Tag + adjacent (×) remove-from-project popconfirm. Split out
 * because the popconfirm needs its own open-state and the App
 * context for toasts — keeping it next to `ProjectMembershipTrailing`
 * inside the same component would balloon the JSX.
 */
function ProjectTagWithRemove({
  conversationId,
  project,
}: {
  conversationId: string
  project: Project
}) {
  const [removeOpen, setRemoveOpen] = useState(false)
  const [, setRemoving] = useState(false)

  const handleRemove = async () => {
    setRemoving(true)
    try {
      await Stores.Projects.detachConversation(project.id, conversationId)
      message.success('Removed from project')
    } catch (err) {
      message.error(
        err instanceof Error ? err.message : 'Failed to remove from project',
      )
    } finally {
      setRemoving(false)
      setRemoveOpen(false)
    }
  }

  // The membership tag IS the remove affordance: it names the project the
  // conversation is filed under, and its × detaches it (via the confirm below) —
  // replacing the former standalone "Remove from project" button. Same hover-
  // reveal wrapper as the other card actions.
  return (
    <>
      <div
        className={`inline-flex items-center transition-opacity ${
          removeOpen
            ? 'opacity-100'
            : 'opacity-0 group-hover:opacity-100 group-focus-within:opacity-100 hover-none:opacity-100'
        }`}
      >
        <Tag
          variant="outline"
          tone="info"
          icon={<FolderOpen />}
          className="max-w-[11rem]"
          title={project.name}
          data-testid="project-trailing-remove-tag"
          onClose={() => setRemoveOpen(true)}
          closeLabel={project.name ? `Remove from ${project.name}` : 'Remove from project'}
        >
          <span className="truncate">{project.name}</span>
        </Tag>
      </div>
      <Confirm
        data-testid="project-trailing-remove-confirm"
        title="Remove from project?"
        description="The conversation becomes unfiled. It is NOT deleted."
        open={removeOpen}
        onOpenChange={setRemoveOpen}
        onConfirm={handleRemove}
        onCancel={() => setRemoveOpen(false)}
        okText="Remove"
        cancelText="Cancel"
      />
    </>
  )
}

/**
 * Hover-mounted per-card decoration. Lazy by virtue of
 * ConversationCard mounting trailing only after first hover, so
 * this component's `useEffect` lookup runs on demand (not on page
 * render).
 *
 * State:
 *   - loading: lookup in flight
 *   - in_project: render "In project: NAME" tag (clickable → /projects/{id})
 *   - unfiled: render "Add to project" button → opens modal
 *
 * Subscribes to project.conversation_attached/detached events so a
 * modal-driven attach immediately flips this card without a reload.
 */
function ProjectMembershipTrailing({
  conversationId,
}: {
  conversationId: string
}) {
  // Projects is admin-only-by-default (projects::read, migration 54). A user
  // without it has no projects, so render no membership badge / "Add to
  // project" affordance at all. Hook order preserved — the gate is applied
  // after all hooks via an early null return below.
  const canUseProjects = usePermission(Permissions.ProjectsRead)
  const [state, setState] = useState<
    { kind: 'loading' } | { kind: 'in_project'; project: Project } | { kind: 'unfiled' }
  >(() => {
    const cached = getCached(conversationId)
    if (cached === null) return { kind: 'unfiled' }
    if (cached && cached.name) return { kind: 'in_project', project: cached }
    return { kind: 'loading' }
  })
  const [modalOpen, setModalOpen] = useState(false)

  // Lookup on mount when the cache doesn't have a usable entry.
  // Routed through `loadProjectForConversation` so concurrent
  // mounts (StrictMode double-mount, fast hover-on/off, etc.)
  // share a single in-flight request.
  useEffect(() => {
    let cancelled = false
    const cached = getCached(conversationId)
    if (cached !== undefined && (cached === null || cached.name)) return
    loadProjectForConversation(conversationId).then(project => {
      if (cancelled) return
      setState(
        project ? { kind: 'in_project', project } : { kind: 'unfiled' },
      )
    })
    return () => {
      cancelled = true
    }
  }, [conversationId])

  // React to attach/detach happening elsewhere.
  useEffect(() => {
    const GROUP = `ProjectMembershipTrailing:${conversationId}`
    const bus = Stores.EventBus

    const offAttached = bus.on(
      'project.conversation_attached',
      async event => {
        if (event.data.conversationId !== conversationId) return
        // Force-refresh: event tells us membership changed; cached
        // value (if any) is stale.
        const project = await loadProjectForConversation(conversationId, true)
        setState(
          project ? { kind: 'in_project', project } : { kind: 'unfiled' },
        )
      },
      GROUP,
    )

    const offDetached = bus.on(
      'project.conversation_detached',
      event => {
        if (event.data.conversationId !== conversationId) return
        setCached(conversationId, null)
        setState({ kind: 'unfiled' })
      },
      GROUP,
    )

    return () => {
      offAttached()
      offDetached()
    }
  }, [conversationId])

  // No projects access → no trailing badge at all (the lookup is skipped too).
  if (!canUseProjects) return null

  if (state.kind === 'loading') {
    return <Spin size="sm" label="Loading" />
  }

  if (state.kind === 'in_project') {
    const project = state.project
    return (
      <ProjectTagWithRemove
        conversationId={conversationId}
        project={project}
      />
    )
  }

  return (
    <>
      {/* Visibility wrapper: hover-only by default; pin visible
          while the modal is open. */}
      <div
        className={`inline-flex items-center transition-opacity ${
          modalOpen
            ? 'opacity-100'
            : 'opacity-0 group-hover:opacity-100 group-focus-within:opacity-100 hover-none:opacity-100'
        }`}
      >
        <Tooltip title="Add to project">
          <Button
            data-testid="project-trailing-add-button"
            variant="outline"
            size="default"
            icon={<CirclePlus />}
            aria-label="Add to project"
            onClick={(e: React.MouseEvent) => {
              e.stopPropagation()
              setModalOpen(true)
            }}
          >
            Add to project
          </Button>
        </Tooltip>
      </div>
      <AddToProjectModal
        open={modalOpen}
        conversationId={conversationId}
        onClose={() => setModalOpen(false)}
      />
    </>
  )
}

/**
 * Hook backing `useConversationMenu` for a single conversation.
 * Returns:
 *   - `items`: menu entries to append to the dropdown (handlers
 *     toggle local state for the overlays below).
 *   - `overlays`: AddToProjectModal + RemoveFromProjectPopconfirm
 *     mounted alongside the trigger so the menu items have something
 *     to open.
 *
 * Lookup is routed through the shared `loadProjectForConversation`
 * dedupe so opening the dropdown shares the cached membership with
 * the card trailing — no second round-trip.
 */
function useProjectMenuContribution(conversation: Conversation): {
  items: DropdownItem[]
  overlays: React.ReactNode
  keepMenuOpen: boolean
} {
  const navigate = useNavigate()
  // Gate: no projects::read → contribute no "Add to project" / "Remove"
  // menu items (projects is admin-only-by-default). Applied at the return
  // to keep hook order stable.
  const canUseProjects = usePermission(Permissions.ProjectsRead)
  const [project, setProject] = useState<Project | null>(() => {
    const cached = getCached(conversation.id)
    return cached && cached.name ? cached : null
  })
  const [loaded, setLoaded] = useState<boolean>(() => {
    const cached = getCached(conversation.id)
    return cached !== undefined && (cached === null || !!cached.name)
  })
  const [addOpen, setAddOpen] = useState(false)

  // Membership lookup. Routed through the deduped helper so
  // simultaneous hovers / dropdown openings share one request.
  useEffect(() => {
    let cancelled = false
    if (loaded) return
    loadProjectForConversation(conversation.id).then(p => {
      if (cancelled) return
      setProject(p)
      setLoaded(true)
    })
    return () => {
      cancelled = true
    }
  }, [conversation.id, loaded])

  // React to attach/detach happening elsewhere so this menu reflects
  // current state next time it opens.
  useEffect(() => {
    const GROUP = `useProjectMenuContribution:${conversation.id}`
    const bus = Stores.EventBus
    const offAttached = bus.on(
      'project.conversation_attached',
      async event => {
        if (event.data.conversationId !== conversation.id) return
        const p = await loadProjectForConversation(conversation.id, true)
        setProject(p)
        setLoaded(true)
      },
      GROUP,
    )
    const offDetached = bus.on(
      'project.conversation_detached',
      event => {
        if (event.data.conversationId !== conversation.id) return
        setCached(conversation.id, null)
        setProject(null)
        setLoaded(true)
      },
      GROUP,
    )
    return () => {
      offAttached()
      offDetached()
    }
  }, [conversation.id])

  // Confirm via Modal.confirm — same pattern as the sidebar
  // widget's delete affordance. Avoids the Popconfirm-inside-Menu-item
  // mess (antd's Menu click delegation fights the bubble path of the
  // popconfirm's Cancel/Confirm buttons; even a state-gate doesn't
  // hold against it). A modal covers the screen, lives in its own
  // portal, owns its own lifecycle, and lets the dropdown close
  // normally on menu-item click.
  const confirmRemove = () => {
    if (!project) return
    void dialog.confirm({
      title: 'Remove from project?',
      description: 'The conversation becomes unfiled. It is NOT deleted.',
      okText: 'Remove',
      cancelText: 'Cancel',
    }).then(async (ok) => {
      if (!ok) return
      try {
        await Stores.Projects.detachConversation(project.id, conversation.id)
        message.success('Removed from project')
      } catch (err) {
        message.error(
          err instanceof Error ? err.message : 'Failed to remove from project',
        )
      }
    })
  }

  const items: DropdownItem[] = project
    ? [
        {
          key: 'project-open',
          icon: <FolderOpen />,
          label: project.name
            ? `Open: ${project.name}`
            : 'Open project',
          onClick: () => navigate(`/projects/${project.id}`),
        },
        {
          key: 'project-remove',
          icon: <CircleMinus />,
          label: 'Remove from project',
          onClick: confirmRemove,
        },
      ]
    : loaded
      ? [
          {
            key: 'project-add',
            icon: <CirclePlus />,
            label: 'Add to project',
            onClick: () => setAddOpen(true),
          },
        ]
      : []

  const overlays = (
    <>
      <AddToProjectModal
        open={addOpen}
        conversationId={conversation.id}
        onClose={() => setAddOpen(false)}
      />
    </>
  )

  // Both the add-modal and the remove-confirm are screen-covering
  // (Modal / Modal.confirm) — the dropdown can close normally when
  // the user clicks an item; the overlay survives on its own.
  // Suppress all project menu items + overlays for users without projects access.
  if (!canUseProjects) return { items: [], overlays: null, keepMenuOpen: false }
  return { items, overlays, keepMenuOpen: false }
}

export default projectExtension
