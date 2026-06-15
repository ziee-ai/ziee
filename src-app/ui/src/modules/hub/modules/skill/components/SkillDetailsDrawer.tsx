import { Descriptions, Drawer, Space, Tag, Typography } from 'antd'
import type { IndexItem } from '@/api-client/types'

const { Paragraph, Title } = Typography

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
  return (
    <Drawer
      open={open}
      onClose={onClose}
      closable={{ closeIcon: true }}
      size="large"
      title={
        <Space>
          <Title level={5} className="!m-0">
            {item.title ?? item.name}
          </Title>
          {item.verified && <Tag color="green">Verified</Tag>}
        </Space>
      }
    >
      <div className="flex flex-col gap-4">
        {item.summary && <Paragraph>{item.summary}</Paragraph>}
        <Descriptions size="small" column={1} bordered>
          <Descriptions.Item label="Name">{item.name}</Descriptions.Item>
          {item.version && (
            <Descriptions.Item label="Version">
              {item.version}
            </Descriptions.Item>
          )}
          {item.tags && item.tags.length > 0 && (
            <Descriptions.Item label="Tags">
              <Space wrap size={4}>
                {item.tags.map(t => (
                  <Tag key={t}>{t}</Tag>
                ))}
              </Space>
            </Descriptions.Item>
          )}
        </Descriptions>
      </div>
    </Drawer>
  )
}
