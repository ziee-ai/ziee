import { useEffect, useRef, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { Tag, Tooltip } from 'antd'
import { FolderOpenOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'

/**
 * Sidekick to the chat header that announces "this conversation lives
 * in project X". Clickable — navigates to the project detail page so
 * the user can browse its knowledge or settings.
 *
 * Renders nothing when the conversation has no project_id (default).
 *
 * Also opportunistically pre-loads ProjectDetail so other components
 * (e.g. assistant picker seed in ConversationPage) can read project
 * defaults without an extra fetch.
 *
 * If the project was deleted out from under the conversation (FK SET
 * NULL behavior on the backend means project_id is normally cleared,
 * but a stale FE state with a deleted project_id IS possible during
 * the brief window before refetch), we render a muted
 * "(deleted project)" chip instead of leaving "In project: Project"
 * forever. Closes audit F4.
 */
export function ConversationProjectChip() {
  const navigate = useNavigate()
  // CRITICAL: every Stores.X.field access goes through the proxy
  // `get` trap which calls useEffect + useStore (2 hooks per
  // property). Reads MUST happen unconditionally at the top of the
  // component, before any early return — otherwise React sees a
  // varying hook count between renders and throws "Rendered more
  // hooks than during the previous render." Read every property used
  // anywhere in this component up front; let JS evaluate the
  // downstream conditionals.
  const { conversation } = Stores.Chat
  const { project, files: projectFiles } = Stores.ProjectDetail

  const projectId = conversation?.project_id ?? null

  /// Tracks load-failure (404) for the current projectId so we can
  /// switch to the "(deleted)" rendering without polling. Resets when
  /// projectId changes.
  const [missing, setMissing] = useState(false)

  /// Monotonic fetch counter — same pattern as RecentConversationsWidget
  /// (audit N9). The previous `cancelled` boolean guarded against the
  /// CURRENT effect's resolver writing state after a re-render, but it
  /// did NOT guard against PRIOR in-flight loads (projectId A → B → A
  /// within ~200ms) winning the race and overwriting newer state.
  /// With a monotonic ref, only the most-recent load can commit.
  const latestFetchIdRef = useRef(0)

  useEffect(() => {
    if (!projectId) {
      setMissing(false)
      return
    }
    if (project?.id === projectId) {
      // Already loaded for this id — clear any prior failure flag.
      setMissing(false)
      return
    }
    latestFetchIdRef.current += 1
    const myFetchId = latestFetchIdRef.current
    ;(async () => {
      try {
        await Stores.ProjectDetail.loadProject(projectId)
        if (myFetchId === latestFetchIdRef.current) setMissing(false)
      } catch {
        if (myFetchId === latestFetchIdRef.current) setMissing(true)
      }
    })()
  }, [projectId, project?.id])

  if (!projectId) return null

  if (missing) {
    return (
      <Tooltip title="This project was deleted; the conversation no longer receives project context.">
        <Tag color="default" className="ml-2 opacity-60">
          (deleted project)
        </Tag>
      </Tooltip>
    )
  }

  // Until ProjectDetail finishes loading, show a neutral chip with the
  // ID-as-fallback so the user still gets the affordance.
  const name = project?.id === projectId ? project?.name : 'Project'
  const fileCount = project?.id === projectId ? projectFiles.length : 0

  // Keyboard-accessible: Tag is rendered as a <span> by antd, so the
  // onClick handler is invisible to keyboard users by default. Add
  // role="button" + tabIndex={0} + Enter/Space activation so screen
  // readers + keyboard navigation reach it (WCAG 2.1 SC 2.1.1).
  const open = () => navigate(`/projects/${projectId}`)
  return (
    <Tooltip title="Open this project">
      <Tag
        color="processing"
        icon={<FolderOpenOutlined />}
        className="cursor-pointer ml-2"
        role="button"
        tabIndex={0}
        aria-label={`Open project ${name}`}
        onClick={open}
        onKeyDown={e => {
          if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault()
            open()
          }
        }}
      >
        In project: {name}
        {fileCount > 0 && ` · ${fileCount} file${fileCount === 1 ? '' : 's'}`}
      </Tag>
    </Tooltip>
  )
}
