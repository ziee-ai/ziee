import type { ReactNode } from 'react'
import { Descriptions, Space, Tag, Paragraph, Title } from '@/components/ui'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import type { IndexItem } from '@/api-client/types'

interface WorkflowDetailsDrawerProps {
  item: IndexItem
  open: boolean
  onClose: () => void
  /** Install actions (mirrors the card) rendered in the drawer footer. */
  footer?: ReactNode
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
  footer,
}: WorkflowDetailsDrawerProps) {
  return (
    <Drawer
      data-testid="hub-workflow-detail-sheet"
      open={open}
      onClose={onClose}
      size={720}
      footer={footer}
      title={
        <Space>
          <Title level={5} className="!m-0">
            {item.title ?? item.name}
          </Title>
          {item.verified && <Tag tone="success" data-testid="hub-workflow-detail-verified-tag">Verified</Tag>}
        </Space>
      }
    >
      <div className="flex flex-col gap-4">
        {item.summary && <Paragraph>{item.summary}</Paragraph>}
        <Descriptions size="sm" column={1} bordered
          data-testid="hub-workflow-detail-descriptions"
          items={[
            { key: 'name', label: 'Name', children: item.name },
            ...(item.version ? [{ key: 'version', label: 'Version', children: item.version }] : []),
            ...(item.tags && item.tags.length > 0 ? [{
              key: 'tags',
              label: 'Tags',
              children: (
                <Space wrap size={4}>
                  {item.tags.map(t => (
                    <Tag key={t} data-testid={`hub-workflow-detail-tag-${t}`}>{t}</Tag>
                  ))}
                </Space>
              )
            }] : [])
          ]}
        />
      </div>
    </Drawer>
  )
}
