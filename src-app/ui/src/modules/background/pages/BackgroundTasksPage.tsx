import { Bot } from 'lucide-react'
import { useEffect } from 'react'

import { ListPagination } from '@/components/common/ListPagination'
import { Empty, ErrorState, message, Spin, Title } from '@ziee/kit'

import { cn } from '@/lib/utils'
import { HeaderBarContainer } from '@/modules/layouts/app-layout/components/HeaderBarContainer'
import { useNativeScroll } from '@/modules/layouts/app-layout/hooks/useNativeScroll'

import { BackgroundRunCard } from '../components/BackgroundRunCard'
import { AppLayout } from '@/modules/layouts/app-layout/appLayout'
import { BackgroundRuns } from '@/modules/background/stores/BackgroundRuns.store'

/**
 * "Background tasks" (ITEM-8 / problem-area #2 "see & manage your background
 * sub-agents beside live chat"). A top-level, app-shell nav destination (so the
 * user keeps the sidebar to hop back to chat) listing the user's detached
 * sub-agent / sandbox-exec runs. Server-paginated over `GET /api/background/runs`;
 * the store refetches live on `sync:workflow_run` so statuses roll to their
 * terminal badge without a manual reload.
 *
 * The FULL result body (`final_output_json`) has no REST getter yet, so a
 * completed run surfaces `has_result` + a link to the bound conversation rather
 * than an inline result view (see the module note / report gap flag).
 */
export function BackgroundTasksPage() {
  useNativeScroll(true)
  const { nativeScroll } = AppLayout
  const { runs, total, currentPage, pageSize, loading, error } =
    BackgroundRuns

  useEffect(() => {
    void BackgroundRuns.loadRuns(1)
  }, [])

  // A refetch error while rows are already on screen (e.g. a sync-driven reload
  // failing) → toast once, then clear so a later change doesn't re-toast the
  // stale error (mirrors the scheduler/projects error routing). The first-load
  // error is rendered inline by ErrorState below.
  useEffect(() => {
    if (error && runs.length > 0) {
      message.error(error)
      BackgroundRuns.clearError()
    }
  }, [error, runs.length])

  return (
    <div
      data-testid="background-tasks-page"
      className={cn(
        'flex flex-col',
        nativeScroll ? 'min-h-dvh' : 'h-full overflow-hidden',
      )}
    >
      <HeaderBarContainer>
        <div className="flex h-full w-full items-center justify-between">
          <Title
            level={4}
            className="!m-0 !leading-tight"
            data-testid="background-tasks-title"
          >
            Background tasks
          </Title>
        </div>
      </HeaderBarContainer>

      <div
        className={cn(
          'flex flex-1 flex-col items-center',
          nativeScroll ? '' : 'overflow-hidden',
        )}
      >
        <div
          className={cn(
            'flex w-full flex-1 flex-col',
            nativeScroll ? '' : 'overflow-hidden',
          )}
        >
          <div
            className={cn(
              'flex flex-col',
              nativeScroll ? '' : 'h-full overflow-y-auto',
            )}
          >
            <div className="flex w-full max-w-4xl flex-col gap-3 self-center px-3 pt-3">
              {loading && runs.length === 0 ? (
                <div className="flex justify-center py-12">
                  <Spin label="Loading background tasks" />
                </div>
              ) : error && runs.length === 0 ? (
                <ErrorState
                  resource="background tasks"
                  description="Your background tasks couldn't be loaded. Check your connection and try again."
                  details={error}
                  onRetry={() => void BackgroundRuns.loadRuns(currentPage)}
                  data-testid="background-tasks-error"
                />
              ) : runs.length === 0 ? (
                <Empty
                  data-testid="background-tasks-empty"
                  icon={<Bot className="size-16" />}
                  title="No background tasks yet"
                  description="When you or the agent launch a background sub-agent, it shows up here — running beside your live chat so you can check, steer, or stop it."
                />
              ) : (
                <>
                  {runs.map(run => (
                    <BackgroundRunCard key={run.id} run={run} />
                  ))}
                  <ListPagination
                    data-testid="background-tasks-pagination"
                    current={currentPage}
                    total={total}
                    pageSize={pageSize}
                    itemNoun="tasks"
                    onChange={page => BackgroundRuns.setPage(page, pageSize)}
                    onPageSizeChange={size => BackgroundRuns.setPage(1, size)}
                    aria-label="Background task pages"
                  />
                </>
              )}
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}
