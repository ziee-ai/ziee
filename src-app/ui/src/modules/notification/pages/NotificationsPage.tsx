import { Check, Trash2 } from 'lucide-react'
import { useEffect } from 'react'
import { useNavigate } from 'react-router-dom'

import {
  Button,
  Card,
  Empty,
  ErrorState,
  Flex,
  Segmented,
  Spin,
  Text,
} from '@/components/ui'
import { Stores } from '@/core/stores'
import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'

/** The full notification inbox at /notifications. */
export function NotificationsPage() {
  const { items, unread, total, page, perPage, unreadOnly, loading, error } =
    Stores.Notifications
  const navigate = useNavigate()

  useEffect(() => {
    void Stores.Notifications.load()
  }, [])

  const open = (n: (typeof items)[number]) => {
    void Stores.Notifications.markRead(n.id)
    if (n.conversation_id) navigate(`/chat/${n.conversation_id}`)
  }

  return (
    <SettingsPageContainer
      title="Notifications"
      subtitle="Background results from your scheduled tasks."
      data-testid="notifications-page"
    >
      <Flex className="mb-3 items-center justify-between">
        <Segmented
          data-testid="notifications-filter"
          value={unreadOnly ? 'unread' : 'all'}
          onChange={v => Stores.Notifications.setUnreadOnly(v === 'unread')}
          options={[
            { label: 'All', value: 'all' },
            { label: `Unread${unread ? ` (${unread})` : ''}`, value: 'unread' },
          ]}
        />
        <Button
          data-testid="notifications-mark-all"
          variant="outline"
          disabled={unread === 0}
          onClick={() => void Stores.Notifications.markAllRead()}
        >
          Mark all read
        </Button>
      </Flex>

      {loading && items.length === 0 ? (
        <Flex className="justify-center py-12">
          <Spin size="lg" label="Loading notifications" />
        </Flex>
      ) : error && items.length === 0 ? (
        <ErrorState
          variant="page"
          resource="notifications"
          details={error}
          onRetry={() => void Stores.Notifications.load()}
          data-testid="notifications-error"
        />
      ) : items.length === 0 ? (
        <Empty
          description="No notifications"
          data-testid="notifications-empty"
        />
      ) : (
        <Flex className="flex-col gap-2">
          {items.map(n => (
            <Card key={n.id} data-testid={`notification-card-${n.id}`}>
              <Flex className="items-start gap-3">
                {!n.read_at && (
                  <span className="mt-1.5 h-2 w-2 shrink-0 rounded-full bg-primary" />
                )}
                <button
                  type="button"
                  className="min-w-0 flex-1 text-left"
                  onClick={() => open(n)}
                  data-testid={`notification-open-${n.id}`}
                >
                  <Text className="font-medium">{n.title}</Text>
                  {n.body && (
                    <Text className="text-muted-foreground text-sm">
                      {n.body}
                    </Text>
                  )}
                  <Text className="text-muted-foreground text-xs">
                    {new Date(n.created_at).toLocaleString()}
                  </Text>
                </button>
                <Flex className="gap-1">
                  {!n.read_at && (
                    <Button
                      data-testid={`notification-read-${n.id}`}
                      variant="ghost"
                      aria-label="Mark read"
                      onClick={() => void Stores.Notifications.markRead(n.id)}
                    >
                      <Check size={16} />
                    </Button>
                  )}
                  <Button
                    data-testid={`notification-delete-${n.id}`}
                    variant="ghost"
                    aria-label="Delete"
                    onClick={() => void Stores.Notifications.remove(n.id)}
                  >
                    <Trash2 size={16} />
                  </Button>
                </Flex>
              </Flex>
            </Card>
          ))}
        </Flex>
      )}

      {total > perPage && (
        <Flex className="justify-center gap-2 pt-4">
          <Button
            data-testid="notifications-prev"
            variant="outline"
            disabled={page <= 1}
            onClick={() => Stores.Notifications.setPage(page - 1)}
          >
            Previous
          </Button>
          <Text className="self-center text-muted-foreground text-sm">
            Page {page} of {Math.max(1, Math.ceil(total / perPage))}
          </Text>
          <Button
            data-testid="notifications-next"
            variant="outline"
            disabled={page >= Math.ceil(total / perPage)}
            onClick={() => Stores.Notifications.setPage(page + 1)}
          >
            Next
          </Button>
        </Flex>
      )}
    </SettingsPageContainer>
  )
}
