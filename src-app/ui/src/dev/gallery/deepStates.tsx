/**
 * Active-conversation deep-state aggregator + frame.
 *
 * Deep-state ENTRIES are now OWNED by the chat module in
 * `src/modules/chat/gallery.tsx` (`gallery.deepStates`) and auto-discovered by
 * the runtime registry. This file keeps the shared FRAME (every deep entry
 * renders the same `ConversationPage` pinned to its `conversationId`) + the SAME
 * export surface (`DEEP_STATE_ENTRIES`/`deepStateBySlug`/`DEEP_STATE_SLUGS`).
 */
import { type ReactNode, Suspense, lazy, useEffect } from 'react'
import { MemoryRouter, Route, Routes } from 'react-router-dom'
import { Text, Title } from '@ziee/kit'
import { AppErrorBoundary } from '@/components/AppErrorBoundary'
import { Loading } from '@/core/components/Loading'
import { useRunInteraction } from './interactions'
import { DEEP_STATE_ENTRIES } from './support/registry'
import type { DeepStateEntry } from './support/types'

export type { DeepStateEntry }
export { DEEP_STATE_ENTRIES }

const ConversationPage = lazy(
  () => import('@/modules/chat/pages/ConversationPage'),
)

export const deepStateBySlug = (slug: string) =>
  DEEP_STATE_ENTRIES.find(e => e.slug === slug)

/** Surface ids each deep entry helps cover (for reference/reporting). */
export const DEEP_STATE_SLUGS = DEEP_STATE_ENTRIES.map(e => e.slug)

const deepTestId = (slug: string) => `gallery-page-${slug}`

/** Renders one deep-state entry: the real ConversationPage + a mount-time seed. */
export function DeepStateFrame({ entry }: { entry: DeepStateEntry }): ReactNode {
  useEffect(() => {
    void entry.setup?.()
  }, [entry])
  // Deep surfaces need their seed + the lazy ConversationPage to settle before an
  // interaction can find its target, so give the recipe a longer settle window.
  useRunInteraction(entry.interactions, 1200)
  return (
    <section
      data-testid={deepTestId(entry.slug)}
      data-gallery-state="deep"
      className="flex flex-col gap-3 border border-border rounded-lg p-4 bg-background"
    >
      <div className="flex flex-col gap-1" data-gallery-chrome>
        <Title level={3}>
          {entry.title}
          <Text tone="muted" className="ml-2 text-sm">
            · deep-state
          </Text>
        </Title>
        <Text tone="muted" className="text-sm">
          gallery-page-{entry.slug} · {entry.note}
        </Text>
      </div>
      <div
        data-gallery-frame
        className="w-full overflow-hidden rounded-md border border-border bg-background"
        style={{ height: 720 }}
      >
        <AppErrorBoundary label={`deep-${entry.slug}`} fallback={() => null}>
          <MemoryRouter initialEntries={[`/chat/${entry.conversationId}`]}>
            <Routes>
              <Route
                path="/chat/:conversationId"
                element={
                  <Suspense fallback={<Loading />}>
                    <ConversationPage />
                  </Suspense>
                }
              />
            </Routes>
          </MemoryRouter>
        </AppErrorBoundary>
      </div>
    </section>
  )
}
