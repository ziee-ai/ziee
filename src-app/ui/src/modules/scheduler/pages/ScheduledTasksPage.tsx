import { CalendarClock, Plus } from 'lucide-react'
import { useEffect, useState } from 'react'

import { Permissions } from '@/api-client/types'
import {
  Button,
  Empty,
  ErrorState,
  message,
  Spin,
  Text,
  Title,
} from '@/components/ui'
import { Can } from '@/core/permissions'
import { Stores } from '@/core/stores'
import { cn } from '@/lib/utils'
import { HeaderBarContainer } from '@/modules/layouts/app-layout/components/HeaderBarContainer'
import { useNativeScroll } from '@/modules/layouts/app-layout/hooks/useNativeScroll'

import { ScheduledTaskCard } from '../components/ScheduledTaskCard'

// Client-side "Load More" paging (the store loads the full set): reveal a page at
// a time, mirroring KnowledgeBasesListPage + ProjectsListPage + the chat list.
const PAGE_SIZE = 12

export function ScheduledTasksPage() {
  useNativeScroll(true)
  const { nativeScroll } = Stores.AppLayout
  const { tasks, loading, error } = Stores.ScheduledTasks
  const [visibleCount, setVisibleCount] = useState(PAGE_SIZE)
  const visibleTasks = tasks.slice(0, visibleCount)
  const hasMore = visibleCount < tasks.length

  useEffect(() => {
    void Stores.ScheduledTasks.loadTasks()
  }, [])

  // Mutation errors (a task already on screen) → toast once, then clear so a
  // later tasks.length change doesn't re-toast the stale error (mirrors the
  // projects/knowledge-base error routing).
  useEffect(() => {
    if (error && tasks.length > 0) {
      message.error(error)
      Stores.ScheduledTasks.clearError()
    }
  }, [error, tasks.length])

  return (
    <div
      data-testid="scheduled-tasks-page"
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
            data-testid="scheduled-tasks-title"
          >
            Scheduled Tasks
          </Title>
          <Can permission={Permissions.SchedulerUse}>
            <Button
              data-testid="scheduled-tasks-new"
              variant="default"
              size="icon"
              icon={<Plus />}
              aria-label="New scheduled task"
              onClick={() => Stores.SchedulerDrawer.openCreate()}
            />
          </Can>
        </div>
      </HeaderBarContainer>

      <div
        className={cn(
          'flex flex-1 flex-col items-center',
          nativeScroll ? '' : 'overflow-hidden',
        )}
      >
        {tasks.length > 0 ? (
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
              <div className="max-w-4xl w-full self-center flex flex-col gap-3 px-3 pt-3">
                {visibleTasks.map(t => (
                  <ScheduledTaskCard key={t.id} task={t} />
                ))}
              </div>

              {/* Paging — "Showing N of M" + Load More (mirrors knowledge-base +
                  projects + the chat conversation list). */}
              <div
                data-testid="scheduled-tasks-paging"
                className="flex flex-col items-center gap-2 px-3 py-3 text-center"
                style={
                  nativeScroll
                    ? {
                        paddingBottom:
                          'calc(env(safe-area-inset-bottom, 0px) + 12px)',
                      }
                    : undefined
                }
              >
                <Text type="secondary" aria-live="polite" role="status">
                  Showing {visibleTasks.length} of {tasks.length} task
                  {tasks.length === 1 ? '' : 's'}
                </Text>
                {hasMore && (
                  <Button
                    data-testid="scheduled-tasks-load-more"
                    onClick={() => setVisibleCount(c => c + PAGE_SIZE)}
                  >
                    Load More
                  </Button>
                )}
              </div>
            </div>
          </div>
        ) : loading ? (
          <div className="m-auto flex justify-center py-12">
            <Spin label="Loading scheduled tasks" />
          </div>
        ) : error ? (
          <div className="w-full max-w-4xl self-center px-3 pt-3">
            <ErrorState
              resource="scheduled tasks"
              description="Your scheduled tasks couldn't be loaded. Check your connection and try again."
              details={error}
              onRetry={() => void Stores.ScheduledTasks.loadTasks()}
              data-testid="scheduled-tasks-error"
            />
          </div>
        ) : (
          <Empty
            data-testid="scheduled-tasks-empty"
            icon={<CalendarClock className="size-16" />}
            title="No scheduled tasks yet"
            description="Create one to have ziee run a prompt or workflow on a schedule while you're away."
          >
            <Can permission={Permissions.SchedulerUse}>
              <Button
                data-testid="scheduled-tasks-empty-create"
                variant="default"
                icon={<Plus />}
                onClick={() => Stores.SchedulerDrawer.openCreate()}
              >
                New task
              </Button>
            </Can>
          </Empty>
        )}
      </div>
    </div>
  )
}
