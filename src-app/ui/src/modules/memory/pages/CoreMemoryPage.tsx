import { useEffect, useState } from 'react'
import { Typography, Select, Spin, Empty } from 'antd'
import { Stores } from '@/core/stores'
import { CoreMemoryBlocksEditor } from '@/modules/memory/components/CoreMemoryBlocksEditor'

const { Title, Paragraph } = Typography

/**
 * /memories/core-memory — pick an assistant, manage your per-assistant
 * core-memory blocks for it. The CoreMemoryBlocksEditor component
 * does the actual CRUD; this page is the host that lets the user
 * choose which assistant to edit.
 *
 * Closes audit R6-#11 — CoreMemoryBlocksEditor was orphaned (no
 * imports), now it's the body of this route.
 */
export function CoreMemoryPage() {
  const [assistantId, setAssistantId] = useState<string | null>(null)
  const [assistants, setAssistants] = useState<
    { id: string; name: string; display_name?: string | null }[]
  >([])
  const [loading, setLoading] = useState(false)

  useEffect(() => {
    setLoading(true)
    fetch('/api/assistants?page=1&per_page=200', { credentials: 'include' })
      .then((r) => r.json())
      .then((body: any) => {
        const rows = body?.assistants ?? body ?? []
        setAssistants(
          rows.map((a: any) => ({
            id: a.id,
            name: a.name,
            display_name: a.display_name,
          })),
        )
      })
      .catch(() => {
        // Non-fatal: editor stays empty and user can't pick. The
        // empty state below explains why.
      })
      .finally(() => setLoading(false))
    // Touch the memories store to keep parity with the other memory
    // pages' init pattern.
    Stores.Memories
  }, [])

  return (
    <div className="max-w-3xl mx-auto p-6">
      <Title level={3}>Assistant core memory</Title>
      <Paragraph type="secondary">
        Core-memory blocks (Letta-style) are prepended to a specific
        assistant&rsquo;s system prompt on every turn. Use them for
        persona, standing instructions, or context you want the
        assistant to always have. Each block is private to you.
      </Paragraph>

      <div className="mb-4">
        <Paragraph strong className="!mb-2">
          Choose an assistant
        </Paragraph>
        {loading ? (
          <Spin />
        ) : assistants.length === 0 ? (
          <Empty description="No assistants yet" image={Empty.PRESENTED_IMAGE_SIMPLE} />
        ) : (
          <Select
            className="w-full"
            placeholder="Pick an assistant"
            value={assistantId ?? undefined}
            onChange={(v) => setAssistantId(v ?? null)}
            options={assistants.map((a) => ({
              value: a.id,
              label: a.display_name || a.name,
            }))}
            showSearch
            optionFilterProp="label"
            allowClear
          />
        )}
      </div>

      {assistantId && <CoreMemoryBlocksEditor assistantId={assistantId} />}
    </div>
  )
}
