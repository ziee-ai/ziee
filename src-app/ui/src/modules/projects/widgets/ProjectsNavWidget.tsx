import { useEffect, useMemo, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { Button, Separator, Empty, Spin, Text } from '@/components/ui'
import { Folder, FolderOpen, Plus } from 'lucide-react'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions, type Project } from '@/api-client/types'
import { DivScrollY } from '@/components/common/DivScrollY'

/**
 * Sidebar widget showing the user's top N projects. Mount-time fetch
 * required per the CLAUDE.md "event-only widget" anti-pattern note —
 * the projects store self-bootstraps via __init__, but the widget also
 * triggers a load on mount in case nothing has accessed the store yet.
 *
 * Each row has a hover-revealed "+" button that navigates to the
 * project's detail page (/projects/{id}); typing in that page's
 * inline ChatInput creates a new conversation, and the project
 * chat extension's afterCreateConversation hook files it into the
 * project.
 */
export function ProjectsNavWidget() {
  const navigate = useNavigate()
  // Permission gate — closes audit Q6. Users without ProjectsRead see
  // no projects sidebar at all (rather than an empty "Projects" header
  // + a useless fetch attempt that would silently 403).
  const canRead = usePermission(Permissions.ProjectsRead)
  const canCreate = usePermission(Permissions.ProjectsCreate)
  const { projects: projectsMap, loading, isInitialized } = Stores.Projects
  // Memoize the sort+slice so unrelated re-renders (hover state below,
  // sidebar layout changes) don't re-walk the whole project list.
  // Recomputes whenever projectsMap identity changes, which is when
  // the store mutates the Map (event-driven invalidation via immer's
  // enableMapSet).
  //
  // Sort uses a NaN-safe comparator: malformed `updated_at` strings
  // (defensive against future codegen quirks or DB corruption) fall
  // back to a stable lexicographic compare instead of producing NaN
  // and an unstable sort order.
  const projects: Project[] = useMemo(
    () =>
      Array.from(projectsMap.values())
        .sort((a, b) => {
          const aT = new Date(a.updated_at).getTime()
          const bT = new Date(b.updated_at).getTime()
          if (isNaN(aT) || isNaN(bT)) {
            return (b.updated_at || '').localeCompare(a.updated_at || '')
          }
          return bT - aT
        })
        .slice(0, 8), // Top 8 by activity; "Browse all" link below for the rest.
    [projectsMap],
  )

  const [hoveredId, setHoveredId] = useState<string | null>(null)
  const [focusedId, setFocusedId] = useState<string | null>(null)

  // Mount fetch even if the store hasn't been touched yet (e.g. user
  // landed straight on /chat without visiting /projects). Skipped
  // when canRead is false to avoid burning an authenticated 403.
  useEffect(() => {
    if (!canRead) return
    if (!isInitialized) {
      void Stores.Projects.loadProjects()
    }
  }, [canRead, isInitialized])

  if (!canRead) {
    return null
  }

  if (loading && !isInitialized) {
    return (
      <div className="flex justify-center items-center py-4">
        <Spin label="Loading" />
      </div>
    )
  }

  return (
    <div className="flex flex-col">
      <div className="px-3 pt-2 pb-1">
        <Text type="secondary" className="text-xs uppercase tracking-wider">
          Projects
        </Text>
      </div>

      {projects.length === 0 ? (
        <div className="px-2 py-3">
          <Empty
            data-testid="project-nav-empty"
            image={<Folder className="text-2xl text-muted-foreground" />}
            description={
              <Text type="secondary" className="text-xs">
                No projects yet
              </Text>
            }
          />
        </div>
      ) : (
        <DivScrollY className="flex-col max-h-64">
          {projects.map(project => {
            // Show the "+" affordance on EITHER hover or keyboard focus
            // — without `focusedId`, Tab-navigating users could never
            // reach the "new chat in this project" button (WCAG 2.1
            // SC 2.1.1 — all functionality must be keyboard-reachable).
            const showPlus =
              (hoveredId === project.id || focusedId === project.id) && canCreate
            const open = () => navigate(`/projects/${project.id}`)
            return (
              <div
                key={project.id}
                className="group relative px-3 py-1.5 cursor-pointer rounded transition-colors hover:bg-black/5 focus:bg-black/5 focus:outline-none"
                onClick={open}
                onMouseEnter={() => setHoveredId(project.id)}
                onMouseLeave={() => setHoveredId(null)}
                onFocus={() => setFocusedId(project.id)}
                onBlur={() => setFocusedId(null)}
                role="button"
                tabIndex={0}
                aria-label={`Open project ${project.name}`}
                onKeyDown={e => {
                  if (e.key === 'Enter' || e.key === ' ') {
                    e.preventDefault()
                    open()
                  }
                }}
                data-project-id={project.id}
              >
                <div className="flex items-center justify-between gap-2">
                  <div className="flex items-center gap-2 min-w-0 flex-1">
                    <FolderOpen className="text-sm shrink-0" />
                    <Text
                      className="text-sm truncate block"
                      title={project.name}
                    >
                      {project.name}
                    </Text>
                  </div>
                  {showPlus && (
                    <Button
                      data-testid={`project-nav-new-chat-button-${project.id}`}
                      variant="ghost"
                      size="sm"
                      icon={<Plus />}
                      aria-label={`New chat in ${project.name}`}
                      onClick={e => {
                        e.stopPropagation()
                        // Go straight to the project detail page;
                        // its inline ChatInput + the project chat
                        // extension's `afterCreateConversation` hook
                        // do the file-into-project on first send.
                        navigate(`/projects/${project.id}`)
                      }}
                    />
                  )}
                </div>
              </div>
            )
          })}
        </DivScrollY>
      )}

      <Separator className="!my-1" />
      <div className="px-2 pb-1">
        <Button
          data-testid="project-nav-all-projects-button"
          variant="ghost"
          icon={<Folder />}
          block
          onClick={() => navigate('/projects')}
        >
          All projects
        </Button>
      </div>
    </div>
  )
}
