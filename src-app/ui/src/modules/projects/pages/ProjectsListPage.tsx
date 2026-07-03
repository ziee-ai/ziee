import { useEffect, useState } from 'react'
import { Button, Spin, Text, Title, message } from '@/components/ui'
import { Folder, FolderPlus, Plus } from 'lucide-react'
import { Stores } from '@/core/stores'
import { Can } from '@/core/permissions'
import { Permissions, type Project } from '@/api-client/types'
import { ProjectCard } from '@/modules/projects/components/ProjectCard'
import { ProjectFormDrawer } from '@/modules/projects/components/ProjectFormDrawer'
import { HeaderBarContainer } from '@/modules/layouts/app-layout/components/HeaderBarContainer'
import { useNativeScroll } from '@/modules/layouts/app-layout/hooks/useNativeScroll'
import { cn } from '@/lib/utils'

export function ProjectsListPage() {
  // Native document-scroll on mobile (iOS toolbar collapse + content under the
  // notch); desktop keeps the fixed inner-scroll shell.
  useNativeScroll(true)
  const { nativeScroll } = Stores.AppLayout
  const { projects: projectsMap, loading, error } = Stores.Projects
  const projects = Array.from(projectsMap.values())
  // Per-card mutation state so the duplicate/delete buttons can show a
  // spinner on the exact card being acted on (the store single-flights
  // globally, but feedback should be card-local).
  const [busy, setBusy] = useState<{
    id: string
    action: 'duplicate' | 'delete'
  } | null>(null)

  // Surface mutation/load failures to the user before clearing, so a
  // failed duplicate/delete isn't swallowed silently.
  useEffect(() => {
    if (error) {
      message.error(error)
      Stores.Projects.clearProjectsError()
    }
  }, [error])

  const handleCreate = () => Stores.ProjectDrawer.openProjectDrawer(null)
  const handleEdit = (project: Project) =>
    Stores.ProjectDrawer.openProjectDrawer(project)

  const handleDuplicate = async (project: Project) => {
    setBusy({ id: project.id, action: 'duplicate' })
    try {
      await Stores.Projects.duplicateProject(project.id)
    } catch (_err) {
      // Surfaced via the store `error` -> message.error effect above.
    } finally {
      setBusy(null)
    }
  }

  const handleDelete = async (project: Project) => {
    setBusy({ id: project.id, action: 'delete' })
    try {
      await Stores.Projects.deleteProject(project.id)
    } catch (_err) {
      // Surfaced via the store `error` -> message.error effect above.
    } finally {
      setBusy(null)
    }
  }

  return (
    <div className={cn('flex flex-col', nativeScroll ? 'min-h-dvh' : 'h-full overflow-hidden')}>
      <HeaderBarContainer>
        <div className="h-full flex items-center justify-between w-full">
          <Title
            level={4}
            className="!m-0 !leading-tight"
            data-testid="project-list-title"
          >
            Projects
          </Title>
          <Can permission={Permissions.ProjectsCreate}>
            <Button
              data-testid="project-list-create-button"
              variant="default"
              size="icon"
              icon={<Plus />}
              onClick={handleCreate}
              aria-label="Create project"
            />
          </Can>
        </div>
      </HeaderBarContainer>

      <div className={cn('flex-1 flex flex-col items-center', nativeScroll ? '' : 'overflow-hidden')}>
        {projects.length > 0 ? (
          <div className={cn('flex flex-1 flex-col w-full', nativeScroll ? '' : 'overflow-hidden')}>
            <div className={cn('flex flex-col', nativeScroll ? '' : 'h-full overflow-y-auto')}>
              <div
                className="max-w-4xl flex flex-wrap gap-3 pt-3 w-full self-center px-3"
                style={nativeScroll ? { paddingBottom: 'calc(env(safe-area-inset-bottom, 0px) + 12px)' } : undefined}
              >
                {projects.map(project => (
                  <div key={project.id} className="min-w-70 flex-1">
                    <ProjectCard
                      project={project}
                      onEdit={handleEdit}
                      onDuplicate={p => void handleDuplicate(p)}
                      onDelete={p => void handleDelete(p)}
                      duplicating={
                        busy?.id === project.id && busy.action === 'duplicate'
                      }
                      deleting={
                        busy?.id === project.id && busy.action === 'delete'
                      }
                    />
                  </div>
                ))}
                <div className="min-w-70 flex-1" />
                <div className="min-w-70 flex-1" />
                <div className="min-w-70 flex-1" />
              </div>
            </div>
          </div>
        ) : loading ? (
          <div className="flex justify-center py-12 m-auto">
            <Spin label="Loading projects" />
          </div>
        ) : (
          <div className="text-center py-12 m-auto" data-testid="project-list-empty">
            <Folder className="size-16 mx-auto mb-4" />
              <Title level={3} className="text-muted-foreground">
                No projects yet
              </Title>
              <Text type="secondary" className="block mb-4">
                Create a project to group related conversations under shared
                instructions and files.
              </Text>
              <Can permission={Permissions.ProjectsCreate}>
                <Button
                  data-testid="project-list-empty-create-button"
                  variant="default"
                  icon={<FolderPlus />}
                  onClick={handleCreate}
                >
                  Create Project
                </Button>
              </Can>
            </div>
        )}
      </div>

      <ProjectFormDrawer />
    </div>
  )
}
