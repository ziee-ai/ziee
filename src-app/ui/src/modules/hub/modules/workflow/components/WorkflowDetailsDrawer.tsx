import { Descriptions, Sheet, Space, Tag, Paragraph, Title } from '@/components/ui'
import type { IndexItem } from '@/api-client/types'

interface WorkflowDetailsDrawerProps {
  item: IndexItem
  open: boolean
  onClose: () => void
}

/**
 * Read-only details for a hub workflow catalog entry. The step DAG
 * ships only inside the bundle (extracted on install), so the catalog
 * view surfaces the curated index metadata.
 */
export function WorkflowDetailsDrawer({
  item,
  open,
  onClose,
}: WorkflowDetailsDrawerProps) {
  return (
    <Sheet
      open={open}
      onOpenChange={(v) => { if (!v) onClose() }}
      side="right"
      className="!max-w-[720px]"
      title={
        <Space>
          <Title level={5} className="!m-0">
            {item.title ?? item.name}
          </Title>
          {item.verified && <Tag tone="success">Verified</Tag>}
        </Space>
      }
    >
      <div className="flex flex-col gap-4">
        {item.summary && <Paragraph>{item.summary}</Paragraph>}
        <Descriptions size="sm" column={1} bordered
          items={[
            { key: 'name', label: 'Name', children: item.name },
            ...(item.version ? [{ key: 'version', label: 'Version', children: item.version }] : []),
            ...(item.tags && item.tags.length > 0 ? [{
              key: 'tags',
              label: 'Tags',
              children: (
                <Space wrap size={4}>
                  {item.tags.map(t => (
                    <Tag key={t}>{t}</Tag>
                  ))}
                </Space>
              )
            }] : [])
          ]}
        />
      </div>
    </Sheet>
  )
}
