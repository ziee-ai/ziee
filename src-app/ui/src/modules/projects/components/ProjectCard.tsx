import { useNavigate } from 'react-router-dom'
import { Button, Card, Flex, Confirm, Tooltip, Text, Title } from '@/components/ui'
import { Copy, Folder, Pencil, Trash2 } from 'lucide-react'
import { usePermission } from '@/core/permissions'
import { Permissions, type Project } from '@/api-client/types'

interface ProjectCardProps {
  project: Project
  onEdit: (project: Project) => void
  onDuplicate: (project: Project) => void
  onDelete: (project: Project) => void
  /** Duplicate mutation for THIS card is in flight (spinner + inert button). */
  duplicating?: boolean
  /** Delete mutation for THIS card is in flight (spinner + inert button). */
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
      className="h-full focus-visible:outline focus-visible:outline-2"
      data-test-project-name={project.name}
      title={
        <div className="flex items-center gap-2 min-w-0">
          <Folder aria-hidden="true" />
          <Title level={5} className="!m-0 truncate">
            {project.name}
          </Title>
        </div>
      }
      extra={
        <Flex gap="small" onClick={stop}>
          {canEdit && (
            <Tooltip content="Edit">
              <Button
                data-testid={`project-card-edit-button-${project.id}`}
                variant="ghost"
                size="sm"
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
                variant="ghost"
                size="sm"
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
            <Confirm
              data-testid={`project-card-delete-confirm-${project.id}`}
              title="Delete project"
              description={`Are you sure you want to delete "${project.name}"? Conversations inside it will be preserved as unfiled.`}
              okText="Delete"
              cancelText="Cancel"
              okButtonProps={{ danger: true }}
              onConfirm={() => {
                onDelete(project)
              }}
              onCancel={stop}
            >
              <Button
                data-testid={`project-card-delete-button-${project.id}`}
                variant="ghost"
                size="sm"
                icon={<Trash2 />}
                loading={deleting}
                aria-label={`Delete ${project.name}`}
                onClick={stop}
              />
            </Confirm>
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
