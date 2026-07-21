import { useEffect, useState } from 'react'
import { Library, Plus } from 'lucide-react'
import { Button, Empty, ErrorState, Spin, Text, Title, message } from '@ziee/kit'
import { Can } from '@/core/permissions'
import { type KnowledgeBase } from '@/api-client/types'
import { Permissions } from '@/api-client/permissions'
import { HeaderBarContainer } from '@/modules/layouts/app-layout/components/HeaderBarContainer'
import { useNativeScroll } from '@/modules/layouts/app-layout/hooks/useNativeScroll'
import { cn } from '@/lib/utils'
import { KnowledgeBaseCard } from '@/modules/knowledge-base/components/KnowledgeBaseCard'
import { KnowledgeBaseFormDrawer } from '@/modules/knowledge-base/components/KnowledgeBaseFormDrawer'
import { KnowledgeBases } from '@/modules/knowledge-base/stores/knowledgeBases'
import { AppLayout } from '@/modules/layouts/app-layout/appLayout'

export function KnowledgeBasesListPage() {
  useNativeScroll(true)
  const { nativeScroll } = AppLayout
  const { items, loading, error } = KnowledgeBases
  const kbs = Array.from(items.values())
  // Client-side "Load More" paging (the store loads the full set): reveal a
  // page at a time, mirroring ProjectsListPage + the chat conversation list.
  const PAGE_SIZE = 12
  const [visibleCount, setVisibleCount] = useState(PAGE_SIZE)
  const visibleKbs = kbs.slice(0, visibleCount)
  const hasMore = visibleCount < kbs.length

  const [drawer, setDrawer] = useState<{ open: boolean; editing: KnowledgeBase | null }>({
    open: false,
    editing: null,
  })
  const [deletingId, setDeletingId] = useState<string | null>(null)

  useEffect(() => {
    if (error && kbs.length > 0) {
      message.error(error)
    }
  }, [error, kbs.length])

  const handleDelete = async (kb: KnowledgeBase) => {
    setDeletingId(kb.id)
    try {
      await KnowledgeBases.remove(kb.id)
    } catch {
      /* surfaced via store error */
    } finally {
      setDeletingId(null)
    }
  }

  return (
    <div className={cn('flex flex-col', nativeScroll ? 'min-h-dvh' : 'h-full overflow-hidden')}>
      <HeaderBarContainer>
        <div className="h-full flex items-center justify-between w-full">
          <Title level={4} className="!m-0 !leading-tight" data-testid="kb-list-title">
            Knowledge
          </Title>
          <Can permission={Permissions.KnowledgeBaseManage}>
            <Button
              data-testid="kb-list-create-button"
              variant="default"
              size="icon"
              icon={<Plus />}
              onClick={() => setDrawer({ open: true, editing: null })}
              aria-label="Create knowledge base"
            />
          </Can>
        </div>
      </HeaderBarContainer>

      <div className={cn('flex-1 flex flex-col items-center', nativeScroll ? '' : 'overflow-hidden')}>
        {kbs.length > 0 ? (
          <div className={cn('flex flex-1 flex-col w-full', nativeScroll ? '' : 'overflow-hidden')}>
            <div className={cn('flex flex-col', nativeScroll ? '' : 'h-full overflow-y-auto')}>
              <div className="max-w-4xl grid grid-cols-1 sm:grid-cols-2 gap-3 pt-3 w-full self-center px-3">
                {visibleKbs.map(kb => (
                  <div key={kb.id} className="min-w-0">
                    <KnowledgeBaseCard
                      knowledgeBase={kb}
                      onEdit={k => setDrawer({ open: true, editing: k })}
                      onDelete={k => void handleDelete(k)}
                      deleting={deletingId === kb.id}
                    />
                  </div>
                ))}
              </div>

              {/* Paging — "Showing N of M" + Load More (mirrors the projects +
                  chat conversation lists). */}
              <div
                data-testid="kb-list-paging"
                className="text-center px-3 py-3 flex flex-col items-center gap-2"
                style={nativeScroll ? { paddingBottom: 'calc(env(safe-area-inset-bottom, 0px) + 12px)' } : undefined}
              >
                <Text type="secondary" aria-live="polite" role="status">
                  Showing {visibleKbs.length} of {kbs.length} knowledge base
                  {kbs.length === 1 ? '' : 's'}
                </Text>
                {hasMore && (
                  <Button
                    data-testid="kb-list-load-more-btn"
                    onClick={() => setVisibleCount(c => c + PAGE_SIZE)}
                  >
                    Load More
                  </Button>
                )}
              </div>
            </div>
          </div>
        ) : loading ? (
          <div className="flex justify-center py-12 m-auto">
            <Spin label="Loading knowledge bases" />
          </div>
        ) : error ? (
          <div className="w-full max-w-4xl self-center px-3 pt-3">
            <ErrorState
              resource="knowledge bases"
              description="Your knowledge bases couldn't be loaded. Check your connection and try again."
              details={error}
              onRetry={() => void KnowledgeBases.load(true)}
              data-testid="kb-list-error"
            />
          </div>
        ) : (
          <Empty
            data-testid="kb-list-empty"
            icon={<Library className="size-16" />}
            title="No knowledge bases yet"
            description="Create a knowledge base and add documents — the agent will retrieve relevant passages from them when you chat."
          >
            <Can permission={Permissions.KnowledgeBaseManage}>
              <Button
                data-testid="kb-list-empty-create-button"
                variant="default"
                icon={<Plus />}
                onClick={() => setDrawer({ open: true, editing: null })}
              >
                Create knowledge base
              </Button>
            </Can>
          </Empty>
        )}
      </div>

      <KnowledgeBaseFormDrawer
        open={drawer.open}
        editing={drawer.editing}
        onClose={() => setDrawer({ open: false, editing: null })}
      />
    </div>
  )
}
