import { useState } from 'react'
import {
  Alert,
  Button,
  Card,
  Confirm,
  Empty,
  ErrorState,
  Flex,
  Separator,
  Spin,
  Switch,
  Text,
  message,
} from '@/components/ui'
import { Trash2, Pencil, FlaskConical, Lock } from 'lucide-react'
import { Permissions, type AuthProviderResponse } from '@/api-client/types'
import { Stores } from '@/core/stores'
import { Can } from '@/core/permissions/Can'
import { AddProviderMenu } from './AddProviderMenu'
import { AuthProviderEditDrawer } from './AuthProviderEditDrawer'
import type { ProviderTemplate } from '../types'

type DrawerState =
  | { mode: 'closed' }
  | { mode: 'create'; template: ProviderTemplate }
  | { mode: 'edit'; existing: AuthProviderResponse }

/// Render the relative time for a TIMESTAMPTZ string. Small dependency-
/// free helper — Intl.RelativeTimeFormat would be heavier than what
/// this UI needs ("2m ago" / "1h ago" / "3d ago").
function relativeTime(iso: string | null | undefined): string {
  if (!iso) return ''
  const then = new Date(iso).getTime()
  if (Number.isNaN(then)) return ''
  const secs = Math.floor((Date.now() - then) / 1000)
  if (secs < 60) return `${secs}s ago`
  if (secs < 3600) return `${Math.floor(secs / 60)}m ago`
  if (secs < 86400) return `${Math.floor(secs / 3600)}h ago`
  return `${Math.floor(secs / 86400)}d ago`
}

export function AuthProvidersListSection() {
  const { providers, loading, error, testingIds } = Stores.AuthProvidersAdmin
  const [drawer, setDrawer] = useState<DrawerState>({ mode: 'closed' })
  const [pendingToggleId, setPendingToggleId] = useState<string | null>(null)

  const onToggle = async (row: AuthProviderResponse, next: boolean) => {
    setPendingToggleId(row.id)
    try {
      await Stores.AuthProvidersAdmin.updateProvider(row.id, { enabled: next })
      message.success(next ? 'Provider enabled' : 'Provider disabled')
    } catch (e: any) {
      // The backend's enable-transition probe failure returns 400 with
      // code AUTH_PROVIDER_ENABLE_FAILED_HEALTH_CHECK and a readable
      // reason. The store emits `auth_provider.auto_disabled` so the
      // Switch snaps back; the toast here just surfaces the reason.
      const reason = e?.message ?? 'Failed to update provider'
      message.error(reason)
    } finally {
      setPendingToggleId(null)
    }
  }

  const onTest = async (row: AuthProviderResponse) => {
    const res = await Stores.AuthProvidersAdmin.testProvider(row.id)
    if (res.ok) {
      message.success(`${row.name}: ${res.message}`)
    } else {
      message.error(`${row.name}: ${res.message}`)
    }
  }

  const onDelete = async (row: AuthProviderResponse) => {
    try {
      await Stores.AuthProvidersAdmin.deleteProvider(row.id)
      message.success(`Deleted ${row.name}`)
    } catch (e: any) {
      message.error(e?.message ?? 'Failed to delete provider')
    }
  }

  const renderLastTestLine = (row: AuthProviderResponse) => {
    if (row.last_test_ok === null || row.last_test_ok === undefined) {
      return (
        <Text type="secondary" className="text-xs block">
          Last test: never
        </Text>
      )
    }
    // The failed case renders as a full Alert below the metadata block
    // (mirrors the LLM repo unhealthy treatment); here we only render
    // the success/inline timestamp.
    if (!row.last_test_ok) return null
    return (
      <Text type="secondary" className="text-xs block">
        Last test: ok ({relativeTime(row.last_test_at)})
      </Text>
    )
  }

  const renderRowActions = (row: AuthProviderResponse) => (
    <Can permission={Permissions.AuthProvidersManage}>
      <Switch
        data-testid={`authprov-toggle-switch-${row.name}`}
        className="!mr-2"
        checked={row.enabled}
        loading={pendingToggleId === row.id}
        onChange={next => onToggle(row, next)}
        tooltip={`Toggle ${row.name}`}
      />
      <Button
        data-testid={`authprov-test-button-${row.name}`}
        variant="ghost"
        icon={<FlaskConical />}
        aria-label={`Test ${row.name}`}
        loading={testingIds.has(row.id)}
        onClick={() => onTest(row)}
      >
        Test
      </Button>
      <Button
        data-testid={`authprov-edit-button-${row.name}`}
        variant="ghost"
        icon={<Pencil />}
        aria-label={`Edit ${row.name}`}
        onClick={() => setDrawer({ mode: 'edit', existing: row })}
      >
        Edit
      </Button>
      <Confirm
        data-testid={`authprov-delete-confirm-${row.name}`}
        title={`Delete ${row.name}?`}
        description="Linked users lose this sign-in method; their accounts remain."
        okText="Delete"
        okButtonProps={{ danger: true }}
        cancelText="Cancel"
        onConfirm={() => onDelete(row)}
      >
        <Button
          data-testid={`authprov-delete-button-${row.name}`}
          variant="ghost"
          icon={<Trash2 />}
          aria-label={`Delete ${row.name}`}
        >
          Delete
        </Button>
      </Confirm>
    </Can>
  )

  return (
    <>
      <Card
        data-testid="authprov-list-card"
        title="Configured providers"
        extra={
          <Can permission={Permissions.AuthProvidersManage}>
            <AddProviderMenu
              existingNames={providers.map(p => p.name)}
              onPick={template => setDrawer({ mode: 'create', template })}
            />
          </Can>
        }
      >
        {loading && providers.length === 0 ? (
          <div className="flex justify-center py-6">
            <Spin label="Loading" />
          </div>
        ) : providers.length === 0 ? (
          error ? (
            <ErrorState
              resource="auth providers"
              description="The configured providers couldn't be loaded. Check your connection and try again."
              details={error}
              onRetry={() => void Stores.AuthProvidersAdmin.loadProviders()}
              data-testid="authprov-list-error"
            />
          ) : (
            <Empty
              data-testid="authprov-empty"
              description="No providers yet"
              image={<Lock className="text-4xl opacity-50" />}
            >
              <Text type="secondary">
                Use the + button to add Google, Microsoft, Apple, or a custom
                OIDC IdP.
              </Text>
            </Empty>
          )
        ) : (
          <Flex className="flex-col gap-4">
            <div>
              {providers.map((row, index) => (
                <div key={row.id} data-testid={`authprov-row-${row.id}`}>
                  <div className="flex items-start gap-3 flex-wrap">
                    <div className="flex-1">
                      {/* flex-wrap (not -reverse): when the row is too narrow the
                          action controls wrap BELOW their provider label, keeping
                          label-then-actions order instead of floating above it. */}
                      <div className="flex items-center gap-2 mb-2 flex-wrap">
                        <div className="flex-1 min-w-48">
                          <Flex align="center" gap="small">
                            <Text className="font-medium">{row.name}</Text>
                            {!row.enabled && (
                              <Text
                                type="secondary"
                                className="text-xs"
                                data-testid={`authprov-disabled-marker-${row.name}`}
                              >
                                (Disabled)
                              </Text>
                            )}
                          </Flex>
                        </div>
                        <div className="flex flex-wrap gap-1 items-center justify-end">
                          {renderRowActions(row)}
                        </div>
                      </div>

                      <div className="space-y-1">
                        <Text type="secondary" className="text-xs block">
                          Provider type: {row.provider_type}
                        </Text>
                        {renderLastTestLine(row)}
                      </div>

                      {row.last_test_ok === false && (
                        <Alert
                          tone="error"
                          data-testid={`authprov-test-failed-alert-${row.name}`}
                          className="!mt-2"
                          title={
                            row.last_test_at
                              ? `Connection test failed at ${new Date(
                                  row.last_test_at,
                                ).toLocaleString()}`
                              : 'Connection test failed'
                          }
                          description={
                            row.last_test_message ?? 'No reason recorded.'
                          }
                        />
                      )}
                    </div>
                  </div>
                  {index < providers.length - 1 && (
                    <Separator className="my-4" />
                  )}
                </div>
              ))}
            </div>
          </Flex>
        )}
      </Card>

      <AuthProviderEditDrawer
        open={drawer.mode !== 'closed'}
        template={drawer.mode === 'create' ? drawer.template : undefined}
        existing={drawer.mode === 'edit' ? drawer.existing : undefined}
        onClose={() => setDrawer({ mode: 'closed' })}
      />
    </>
  )
}
