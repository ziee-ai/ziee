import { DeleteOutlined } from '@ant-design/icons'
import { App, Button, Card, Popconfirm, Space, Typography } from 'antd'
import type { BibliographyEntry } from '@/api-client/types'
import { Stores } from '@/core/stores'
import { VerificationBadge } from './VerificationBadge'

const { Text, Paragraph } = Typography

/** Pull a compact author list out of the entry's CSL-JSON. */
function authorLine(csl: unknown): string {
  if (!csl || typeof csl !== 'object') return ''
  const authors = (csl as Record<string, unknown>).author
  if (!Array.isArray(authors)) return ''
  const names = authors
    .map(a => {
      if (!a || typeof a !== 'object') return ''
      const o = a as Record<string, unknown>
      const family = typeof o.family === 'string' ? o.family : ''
      const given = typeof o.given === 'string' ? o.given : ''
      const literal = typeof o.literal === 'string' ? o.literal : ''
      return family ? `${family}${given ? ` ${given[0]}` : ''}` : literal
    })
    .filter(Boolean)
  if (names.length === 0) return ''
  return names.length > 4 ? `${names.slice(0, 4).join(', ')}, et al.` : names.join(', ')
}

export function CitationCard({
  entry,
  canManage,
}: {
  entry: BibliographyEntry
  // Required (no default) so a caller can't accidentally fail-OPEN and render
  // an ungated Delete; both call sites pass the resolved permission.
  canManage: boolean
}) {
  const { message } = App.useApp()
  const handleDelete = async () => {
    try {
      await Stores.Citations.remove(entry.id)
    } catch (e) {
      message.error(e instanceof Error ? e.message : 'Delete failed')
    }
  }
  const authors = authorLine(entry.csl_json)
  const meta = [authors, entry.year ? String(entry.year) : '']
    .filter(Boolean)
    .join(' · ')

  return (
    <Card size="small" style={{ marginBottom: 8 }}>
      <Space direction="vertical" size={2} style={{ width: '100%' }}>
        <Space align="center" style={{ justifyContent: 'space-between', width: '100%' }}>
          <Space size={8}>
            <VerificationBadge status={entry.verification_status} />
            <Text
              code
              copyable={{ tooltips: ['Copy citation key', 'Copied'] }}
            >
              {entry.citation_key}
            </Text>
          </Space>
          {canManage && (
            <Popconfirm
              title="Delete from library?"
              description="Removes it from the library and every project."
              okButtonProps={{ danger: true }}
              onConfirm={handleDelete}
            >
              <Button
                size="small"
                danger
                type="text"
                aria-label={`Delete ${entry.citation_key}`}
                icon={<DeleteOutlined />}
              />
            </Popconfirm>
          )}
        </Space>
        <Text strong>{entry.title || '(untitled)'}</Text>
        {meta && <Text type="secondary">{meta}</Text>}
        {entry.doi && (
          <Paragraph style={{ margin: 0 }}>
            <a
              href={`https://doi.org/${entry.doi}`}
              target="_blank"
              rel="noreferrer"
            >
              doi:{entry.doi}
            </a>
          </Paragraph>
        )}
      </Space>
    </Card>
  )
}
