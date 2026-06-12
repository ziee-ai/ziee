import { useEffect, useRef } from 'react'
import { Card, Flex, Progress, Spin, Typography } from 'antd'
import { Stores } from '@/core/stores'

const { Paragraph } = Typography

/**
 * Renders rebuild progress cards. Self-hides unless an embedding
 * re-embed or FTS-dictionary rebuild is in flight. Polls each
 * status endpoint every 2s.
 *
 * Permission gating: lives inside the admin page which is itself
 * gated on `MemoryAdminRead` — no per-section read check needed.
 */
export function RebuildStatusSection() {
  const { rebuildStatus, ftsRebuildStatus, settings } = Stores.MemoryAdmin
  const rebuildTotalRef = useRef<number>(0)

  // Poll embedding rebuild while in flight.
  useEffect(() => {
    if (!rebuildStatus?.in_progress) return
    const id = setInterval(() => {
      Stores.MemoryAdmin.loadRebuildStatus()
    }, 2000)
    return () => clearInterval(id)
  }, [rebuildStatus?.in_progress])

  // Poll FTS rebuild while in flight.
  useEffect(() => {
    if (!ftsRebuildStatus?.in_progress) return
    const id = setInterval(() => {
      Stores.MemoryAdmin.loadFtsRebuildStatus()
    }, 2000)
    return () => clearInterval(id)
  }, [ftsRebuildStatus?.in_progress])

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

  const embeddingInProgress = rebuildStatus?.in_progress ?? false
  const ftsInProgress = ftsRebuildStatus?.in_progress ?? false

  if (!embeddingInProgress && !ftsInProgress) return null

  const percent =
    rebuildTotalRef.current > 0 && rebuildStatus
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
    <>
      {embeddingInProgress && rebuildStatus && (
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
      )}
      {ftsInProgress && (
        <Card
          title={
            <Flex align="center" gap={8}>
              <Spin size="small" />
              <span>Rebuilding full-text search index</span>
            </Flex>
          }
        >
          <Paragraph type="secondary" className="!mb-2 text-sm">
            Rewriting <code>user_memories.content_tsv</code> with the{' '}
            <code>{settings?.fts_dictionary ?? '...'}</code> dictionary.
            Lexical retrieval continues to work using the prior column
            until the rebuild commits.
          </Paragraph>
          <Progress percent={undefined} status="active" />
        </Card>
      )}
    </>
  )
}
