import { useEffect, useRef } from 'react'
import { Card, Flex, Progress, Spin } from '@ziee/kit'
import { Stores } from '@ziee/framework/stores'

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
      (rebuildStatus?.pending_count ?? 0) > rebuildTotalRef.current
    ) {
      rebuildTotalRef.current = rebuildStatus?.pending_count ?? 0
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
              ((rebuildTotalRef.current - (rebuildStatus?.pending_count ?? 0)) /
                rebuildTotalRef.current) *
                100,
            ),
          ),
        )
      : 0

  return (
    <>
      {embeddingInProgress && rebuildStatus && (
        <Card
          data-testid="memory-rebuild-embedding-card"
          title={
            <Flex align="center" gap="small">
              <Spin size="sm" label="Re-embedding memories" />
              <span>Re-embedding memories</span>
            </Flex>
          }
        >
          <p className="text-sm text-secondary-foreground/70 mb-2">
            Running{' '}
            {rebuildStatus?.model_name ? (
              <code>{rebuildStatus.model_name}</code>
            ) : (
              'the configured embedding model'
            )}{' '}
            against every stored memory. Retrieval may return fewer results
            until this finishes; new memories created during the rebuild
            are picked up automatically.
          </p>
          <Progress value={percent} aria-label="Rebuild progress" data-testid="memory-rebuild-embedding-progress" />
          <p className="text-xs text-secondary-foreground/70 mb-0" data-testid="memory-rebuild-embedding-remaining">
            {rebuildStatus?.pending_count} memor
            {rebuildStatus?.pending_count === 1 ? 'y' : 'ies'} remaining.
          </p>
        </Card>
      )}
      {ftsInProgress && (
        <Card
          data-testid="memory-rebuild-fts-card"
          title={
            <Flex align="center" gap="small">
              <Spin size="sm" label="Rebuilding full-text search index" />
              <span>Rebuilding full-text search index</span>
            </Flex>
          }
        >
          <p className="text-sm text-secondary-foreground/70 mb-2">
            Rewriting <code>user_memories.content_tsv</code> with the{' '}
            <code>{settings?.fts_dictionary ?? '...'}</code> dictionary.
            Lexical retrieval continues to work using the prior column
            until the rebuild commits.
          </p>
          <Progress value={0} aria-label="Rebuild progress" data-testid="memory-rebuild-fts-progress" />
        </Card>
      )}
    </>
  )
}
