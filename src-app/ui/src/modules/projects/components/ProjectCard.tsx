import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { Button, Card, Flex, Confirm, Tooltip, Text, Title } from '@/components/ui'
import { Copy, Pencil, Trash2 } from 'lucide-react'
import { usePermission } from '@/core/permissions'
import { Permissions, type Project } from '@/api-client/types'
import { cn } from '@/lib/utils'

interface ProjectCardProps {
  project: Project
  onEdit: (project: Project) => void
  onDuplicate: (project: Project) => void
  onDelete: (project: Project) => void
  /** Duplicate request in flight for this card — shows a button spinner. */
  duplicating?: boolean
  /** Delete request in flight for this card — shows a button spinner. */
  deleting?: boolean
}

/**
 * Project card. Edit / Duplicate / Delete surface as inline icon
 * buttons in the card header (matches the canonical pattern from
 * `ui-consistency-patterns.md`: destructive actions are inline +
 * Popconfirm, not kebab-Dropdown + Modal.confirm). Each button is
 * permission-gated so users without the corresponding perm don't see
 * the action at all (audit Q4).
 */
export function ProjectCard({
  project,
  onEdit,
  onDuplicate,
  onDelete,
  duplicating = false,
  deleting = false,
}: ProjectCardProps) {
  const navigate = useNavigate()
  const [deleteOpen, setDeleteOpen] = useState(false)
  const canEdit = usePermission(Permissions.ProjectsEdit)
  const canCreate = usePermission(Permissions.ProjectsCreate)
  const canRead = usePermission(Permissions.ProjectsRead)
  const canDelete = usePermission(Permissions.ProjectsDelete)
  // POST /projects/{id}/duplicate requires RequirePermissions<(
  //   ProjectsCreate, ProjectsRead)> on the backend. The FE must
  // match exactly — gating on Edit (the previous proxy) would BOTH
  // hide the button from a user who could duplicate (has Create+Read
  // but no Edit) AND show it to a user who couldn't (has Create+Edit
  // but no Read). Mirror the backend predicate precisely.
  const canDuplicate = canCreate && canRead

  // Stops both button-click bubble + Confirm onCancel passthrough.
  // Confirm passes `MouseEvent<HTMLElement> | undefined` to onCancel;
  // we accept any event-like object with stopPropagation defensively.
  const stop = (e?: { stopPropagation?: () => void }) => {
    e?.stopPropagation?.()
  }

  const handleOpen = () => navigate(`/projects/${project.id}`)

  return (
    <Card
      data-testid={`project-card-${project.id}`}
      hoverable
      onClick={handleOpen}
      role="button"
      tabIndex={0}
      aria-label={`Open project ${project.name}`}
      onKeyDown={e => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault()
          handleOpen()
        }
      }}
      className="group h-full focus-visible:outline focus-visible:outline-2"
      data-test-project-name={project.name}
      title={
        <div className="flex items-start gap-2 min-w-0">
          {/* Wrap to two lines before ellipsizing — a single-line truncate
              ("UI Sh…") wasted the card width on tablet/mobile. */}
          <Title level={5} className="!m-0 !font-normal !text-sm line-clamp-2 [overflow-wrap:anywhere]">
            {project.name}
          </Title>
        </div>
      }
      extra={
        <Flex
          gap="small"
          onClick={stop}
          // Hidden until the card is hovered/focused; always shown on touch
          // (no hover). Pinned visible while the delete confirm is open.
          className={cn(
            'transition-opacity',
            deleteOpen
              ? 'opacity-100'
              : 'opacity-0 group-hover:opacity-100 group-focus-within:opacity-100 hover-none:opacity-100',
          )}
        >
          {canEdit && (
            <Tooltip content="Edit">
              <Button
                data-testid={`project-card-edit-button-${project.id}`}
                variant="outline"
                size="icon"
                icon={<Pencil />}
                aria-label={`Edit ${project.name}`}
                onClick={(e: React.MouseEvent) => {
                  stop(e)
                  onEdit(project)
                }}
              />
            </Tooltip>
          )}
          {canDuplicate && (
            <Tooltip content="Duplicate">
              <Button
                data-testid={`project-card-duplicate-button-${project.id}`}
                variant="outline"
                size="icon"
                icon={<Copy />}
                loading={duplicating}
                aria-label={`Duplicate ${project.name}`}
                onClick={(e: React.MouseEvent) => {
                  stop(e)
                  onDuplicate(project)
                }}
              />
            </Tooltip>
          )}
          {canDelete && (
            <>
              <Tooltip content="Delete">
                <Button
                  data-testid={`project-card-delete-button-${project.id}`}
                  variant="outline"
                  size="icon"
                  icon={<Trash2 />}
                  loading={deleting}
                  aria-label={`Delete ${project.name}`}
                  onClick={(e: React.MouseEvent) => {
                    stop(e)
                    setDeleteOpen(true)
                  }}
                />
              </Tooltip>
              <Confirm
                data-testid={`project-card-delete-confirm-${project.id}`}
                open={deleteOpen}
                onOpenChange={setDeleteOpen}
                title="Delete project"
                description={`Are you sure you want to delete "${project.name}"? Conversations inside it will be preserved as unfiled.`}
                okText="Delete"
                cancelText="Cancel"
                okButtonProps={{ danger: true }}
                onConfirm={() => {
                  onDelete(project)
                }}
              />
            </>
          )}
        </Flex>
      }
    >
      <div className="min-h-12">
        <Text type="secondary" className="line-clamp-3 block">
          {project.description || (
            <span className="italic">No description</span>
          )}
        </Text>
      </div>
    </Card>
  )
}
