import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { Button, Card, Confirm, Flex, Tag, Text, Title, Tooltip } from '@ziee/kit'
import { Pencil, Trash2 } from 'lucide-react'
import { usePermission } from '@/core/permissions'
import { type KnowledgeBase } from '@/api-client/types'
import { Permissions } from '@/api-client/permissions'
import { cn } from '@/lib/utils'

interface Props {
  knowledgeBase: KnowledgeBase
  onEdit: (kb: KnowledgeBase) => void
  onDelete: (kb: KnowledgeBase) => void
  deleting?: boolean
}

/** Doc-count + indexing status chip derived from the KB's indexing_summary. */
function statusChip(kb: KnowledgeBase) {
  const s = kb.indexing_summary
  if (s.failed > 0)
    return { tone: 'error' as const, label: `${s.failed} failed` }
  if (s.indexing + s.pending > 0)
    return { tone: 'warning' as const, label: `${s.indexing + s.pending} indexing` }
  if (s.no_text > 0)
    return { tone: 'default' as const, label: `${s.no_text} no text` }
  if (s.total > 0) return { tone: 'success' as const, label: 'All indexed' }
  return null
}

export function KnowledgeBaseCard({ knowledgeBase, onEdit, onDelete, deleting }: Props) {
  const navigate = useNavigate()
  const canManage = usePermission(Permissions.KnowledgeBaseManage)
  const [deleteOpen, setDeleteOpen] = useState(false)

  const stop = (e: React.MouseEvent) => e.stopPropagation()
  const open = () => navigate(`/knowledge/${knowledgeBase.id}`)
  const chip = statusChip(knowledgeBase)

  return (
    <Card
      data-testid={`kb-card-${knowledgeBase.id}`}
      hoverable
      onClick={open}
      role="button"
      tabIndex={0}
      aria-label={`Open knowledge base ${knowledgeBase.name}`}
      onKeyDown={e => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault()
          open()
        }
      }}
      className="group h-full focus-visible:outline focus-visible:outline-2"
      title={
        <div className="flex items-start gap-2 min-w-0">
          {/* Mirror ProjectCard's title typography exactly (light weight,
              small, two-line wrap) so the two list-grid entity cards read as
              one system — no leading icon, matching the project card. */}
          <Title level={5} className="!m-0 !font-normal !text-sm line-clamp-2 [overflow-wrap:anywhere]">
            {knowledgeBase.name}
          </Title>
        </div>
      }
      extra={
        canManage && (
          <Flex
            gap="small"
            onClick={stop}
            className={cn(
              'transition-opacity',
              deleteOpen
                ? 'opacity-100'
                : 'opacity-0 group-hover:opacity-100 group-focus-within:opacity-100 hover-none:opacity-100',
            )}
          >
            <Tooltip content="Edit">
              <Button
                data-testid={`kb-card-edit-button-${knowledgeBase.id}`}
                variant="outline"
                size="icon"
                icon={<Pencil />}
                aria-label={`Edit ${knowledgeBase.name}`}
                onClick={(e: React.MouseEvent) => {
                  stop(e)
                  onEdit(knowledgeBase)
                }}
              />
            </Tooltip>
            <Tooltip content="Delete">
              <Button
                data-testid={`kb-card-delete-button-${knowledgeBase.id}`}
                variant="outline"
                size="icon"
                icon={<Trash2 />}
                loading={deleting}
                aria-label={`Delete ${knowledgeBase.name}`}
                onClick={(e: React.MouseEvent) => {
                  stop(e)
                  setDeleteOpen(true)
                }}
              />
            </Tooltip>
            <Confirm
              data-testid={`kb-card-delete-confirm-${knowledgeBase.id}`}
              open={deleteOpen}
              onOpenChange={setDeleteOpen}
              title="Delete knowledge base"
              description={`Delete "${knowledgeBase.name}"? Its documents (and their files) are kept — only the collection is removed.`}
              okText="Delete"
              cancelText="Cancel"
              okButtonProps={{ danger: true }}
              onConfirm={() => onDelete(knowledgeBase)}
            />
          </Flex>
        )
      }
    >
      <div className="min-h-12">
        <Text type="secondary" className="line-clamp-2 block">
          {knowledgeBase.description || <span className="italic">No description</span>}
        </Text>
      </div>
      <Flex gap="small" align="center" className="mt-3 flex-wrap">
        <Tag data-testid={`kb-card-doc-count-${knowledgeBase.id}`}>
          {knowledgeBase.document_count} document
          {knowledgeBase.document_count === 1 ? '' : 's'}
        </Tag>
        {chip && (
          <Tag data-testid={`kb-card-status-${knowledgeBase.id}`} tone={chip.tone}>
            {chip.label}
          </Tag>
        )}
      </Flex>
    </Card>
  )
}
