import { useEffect, useRef } from 'react'
import { Card, Flex, Progress, Spin, Typography } from 'antd'
import { Stores } from '@/core/stores'

const { Paragraph } = Typography

/**
 * Renders the embedding-rebuild progress card. Only visible while a
 * rebuild is in flight. Polls `/api/memory/admin-settings/rebuild-status`
 * every 2s to update the progress bar.
 *
 * Permission gating: lives inside the admin page which is itself
 * gated on `MemoryAdminRead` — no per-section read check needed.
 */
export function RebuildStatusSection() {
  const { rebuildStatus } = Stores.MemoryAdmin
  const rebuildTotalRef = useRef<number>(0)

  // Poll while in flight. 2s cadence — fast enough to feel responsive,
  // slow enough that the per-row worker can do real work between polls
  // without spamming the DB.
  useEffect(() => {
    if (!rebuildStatus?.in_progress) return
    const id = setInterval(() => {
      Stores.MemoryAdmin.loadRebuildStatus()
    }, 2000)
    return () => clearInterval(id)
  }, [rebuildStatus?.in_progress])

  // Snapshot the total when a rebuild starts so % is meaningful.
  useEffect(() => {
    if (
      rebuildStatus?.in_progress &&
      rebuildStatus.pending_count > rebuildTotalRef.current
    ) {
      rebuildTotalRef.current = rebuildStatus.pending_count
    }
    if (!rebuildStatus?.in_progress && rebuildStatus?.pending_count === 0) {
      rebuildTotalRef.current = 0
    }
  }, [rebuildStatus])

  if (!rebuildStatus?.in_progress) return null

  const percent =
    rebuildTotalRef.current > 0
      ? Math.max(
          0,
          Math.min(
            100,
            Math.round(
              ((rebuildTotalRef.current - rebuildStatus.pending_count) /
                rebuildTotalRef.current) *
                100,
            ),
          ),
        )
      : undefined

  return (
    <Card
      title={
        <Flex align="center" gap={8}>
          <Spin size="small" />
          <span>Re-embedding memories</span>
        </Flex>
      }
    >
      <Paragraph type="secondary" className="!mb-2 text-sm">
        Running{' '}
        {rebuildStatus.model_name ? (
          <code>{rebuildStatus.model_name}</code>
        ) : (
          'the configured embedding model'
        )}{' '}
        against every stored memory. Retrieval may return fewer results
        until this finishes; new memories created during the rebuild
        are picked up automatically.
      </Paragraph>
      <Progress percent={percent} status="active" />
      <Paragraph type="secondary" className="!mb-0 text-xs">
        {rebuildStatus.pending_count} memor
        {rebuildStatus.pending_count === 1 ? 'y' : 'ies'} remaining.
      </Paragraph>
    </Card>
  )
}
