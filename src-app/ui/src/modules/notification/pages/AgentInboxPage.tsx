import { useEffect, useState } from 'react'
import { useNavigate } from 'react-router-dom'

import type { NotificationRendererCtx } from '@ziee/framework/notification'

import { Button, Card, Empty, ErrorState, Flex, Spin } from '@ziee/kit'
import { NotificationItem } from '@ziee/notification-ui'

import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'

import { AGENT_INBOX_KINDS, resolveAgentInboxKinds } from '../agentInboxKinds'
import { NotificationsView as Notifications } from '@/modules/notification/stores/Notifications.store'

/**
 * The "Background results" inbox — a FOCUSED, agent-scoped view over the shared
 * `Notifications` inbox (ITEM-26 / DEC-65: a FE composition, NOT a forked
 * store). It reuses the SDK notification store + actions + per-kind renderer seam
 * verbatim; it only narrows the list to the agent/background kinds (a background
 * sub-agent group finishing, a scheduled loop returning) so a user who kicked off
 * unattended work sees the results when they land.
 *
 * The go-to-result affordance is the whole-row click: it marks the row read and
 * routes through the app's `onNavigate` seam, which opens the `conversation_id`
 * the result landed in (`/chat/:id`) — the JTBD.
 */
export function AgentInboxPage() {
  // Reactive store reads (subscribe on the proxy getter — legal in render).
  const { items, loading, error } = Notifications
  const navigate = useNavigate()

  // The effective agent/background kind set: the typed constant, augmented once
  // from the live kind registry (`GET /api/notifications/kinds`) where a matching
  // kind is advertised. Page-local UI state (the registry has no store); degrades
  // to the constant on any fetch failure.
  const [agentKinds, setAgentKinds] = useState<Set<string>>(
    () => new Set<string>(AGENT_INBOX_KINDS),
  )
  useEffect(() => {
    let alive = true
    void resolveAgentInboxKinds().then(set => {
      if (alive) setAgentKinds(set)
    })
    return () => {
      alive = false
    }
  }, [])

  // Refresh the shared inbox on entry, then RECONCILE on an interval while the page
  // is open. The live `sync:notification` push is the fast path, but it is
  // best-effort: the per-user sync SSE stream can drop/flap (a reconnect can miss
  // the notify-only frame, which has no server replay), so a background result
  // could otherwise never appear on an open inbox until a manual reload. The
  // notifications are DURABLE rows, so a modest periodic reload is a correct
  // eventual-consistency backstop (not a "manual reload") that guarantees a landed
  // result surfaces within one interval regardless of live-push delivery.
  useEffect(() => {
    void Notifications.load()
    const id = globalThis.setInterval(() => {
      void Notifications.load()
    }, 10_000)
    return () => globalThis.clearInterval(id)
  }, [])

  const list = (items ?? []).filter(n => agentKinds.has(n.kind))
  const unread = list.filter(n => !n.read_at).length

  // Per-kind renderer context (seam). `close` is a no-op on a full page.
  const ctx: NotificationRendererCtx = {
    markRead: (id: string) => void Notifications.markRead(id),
    remove: (id: string) => void Notifications.remove(id),
    close: () => {},
  }

  // Whole-row select → mark read + the app-supplied navigation seam (opens the
  // conversation the result landed in). `onNavigate` is undefined only if the app
  // bound no seam — then rows aren't whole-row clickable (per-kind actions still
  // work), matching the sibling inbox.
  const onNavigate = Notifications.onNavigate
  const onSelect = onNavigate
    ? (id: string) => {
        const n = list.find(r => r.id === id)
        if (!n) return
        void Notifications.markRead(id)
        onNavigate(n, to => navigate(to))
      }
    : undefined

  // Scoped "mark all read": only the background-result rows currently shown (the
  // shared `markAllRead` would also clear unrelated notifications). Reuses the
  // existing per-row `markRead` action — no forked store, no new endpoint.
  const markAllBackgroundRead = () => {
    for (const n of list) {
      if (!n.read_at) void Notifications.markRead(n.id)
    }
  }

  return (
    <SettingsPageContainer
      title="Background results"
      subtitle="Results from the background sub-agents and scheduled loops you started — open one to jump to where it landed."
      data-testid="agent-inbox-page"
    >
      <Flex className="mb-3 items-center justify-end">
        <Button
          data-testid="agent-inbox-mark-all"
          variant="outline"
          disabled={unread === 0}
          onClick={markAllBackgroundRead}
        >
          {unread ? `Mark all read (${unread})` : 'Mark all read'}
        </Button>
      </Flex>

      {loading && list.length === 0 ? (
        <Flex className="justify-center py-12">
          <Spin size="lg" label="Loading background results" />
        </Flex>
      ) : error && list.length === 0 ? (
        <ErrorState
          variant="page"
          resource="background results"
          details={error}
          onRetry={() => void Notifications.load()}
          data-testid="agent-inbox-error"
        />
      ) : list.length === 0 ? (
        <Empty
          description="No background results yet"
          data-testid="agent-inbox-empty"
        />
      ) : (
        <Flex className="flex-col gap-2">
          {list.map(n => (
            <Card key={n.id} data-testid={`agent-inbox-card-${n.id}`}>
              <NotificationItem
                n={n}
                ctx={ctx}
                testidPrefix="agent-notification"
                onSelect={onSelect ? () => onSelect(n.id) : undefined}
              />
            </Card>
          ))}
        </Flex>
      )}
    </SettingsPageContainer>
  )
}
