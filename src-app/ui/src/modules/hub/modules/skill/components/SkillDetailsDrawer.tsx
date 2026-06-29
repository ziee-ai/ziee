import { Descriptions, Sheet, Space, Tag, Paragraph, Title } from '@/components/ui'
import type { DescriptionsItem } from '@/components/ui'
import type { IndexItem } from '@/api-client/types'

interface SkillDetailsDrawerProps {
  item: IndexItem
  open: boolean
  onClose: () => void
}

/**
 * Read-only details for a hub skill catalog entry. The full SKILL.md
 * body isn't shipped in the index — that lands on disk only after
 * install — so this surfaces the curated index metadata (summary,
 * tags, version, verified).
 */
export function SkillDetailsDrawer({
  item,
  open,
  onClose,
}: SkillDetailsDrawerProps) {
  const items: DescriptionsItem[] = [
    { key: 'name', label: 'Name', children: item.name },
    ...(item.version ? [{ key: 'version', label: 'Version', children: item.version }] : []),
    ...(item.tags && item.tags.length > 0
      ? [{
          key: 'tags',
          label: 'Tags',
          children: (
            <Space wrap size="xs">
              {item.tags.map(t => (
                <Tag key={t} data-testid={`hub-skill-detail-tag-${t}`}>{t}</Tag>
              ))}
            </Space>
          ),
        }]
      : []),
  ]
  return (
    <Sheet
      data-testid="hub-skill-detail-sheet"
      open={open}
      onOpenChange={(v) => { if (!v) onClose() }}
      className="!max-w-[600px]"
      title={
        <Space>
          <Title level={5} className="!m-0">
            {item.title ?? item.name}
          </Title>
          {item.verified && <Tag tone="success" data-testid="hub-skill-detail-verified-tag">Verified</Tag>}
        </Space>
      }
    >
      <div className="flex flex-col gap-4">
        {item.summary && <Paragraph>{item.summary}</Paragraph>}
        <Descriptions size="sm" column={1} bordered items={items} data-testid="hub-skill-detail-descriptions" />
      </div>
    </Sheet>
  )
}
