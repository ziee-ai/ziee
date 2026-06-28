import { Alert, Button, Card, Flex, Spin } from 'antd'

interface SettingsSectionStatusProps {
  /** Card title, kept identical to the loaded section so the page layout is stable. */
  title: string
  /** Load error message, if the settings fetch failed. */
  error?: string | null
  /** Retry callback — re-invokes the store's load action. */
  onRetry?: () => void
}

/**
 * Shared fallback body for settings sections whose data is still loading or
 * failed to load. Previously each section did `if (!settings) return null`,
 * which left a permanently-blank card on a load failure with no way to retry.
 * Render this instead: an error+Retry alert when `error` is set, otherwise a
 * centered spinner while the fetch is in flight.
 */
export function SettingsSectionStatus({
  title,
  error,
  onRetry,
}: SettingsSectionStatusProps) {
  return (
    <Card title={title}>
      {error ? (
        <Alert
          type="error"
          showIcon
          message={`Failed to load ${title.toLowerCase()} settings`}
          description={error}
          action={
            onRetry ? (
              <Button size="small" onClick={onRetry}>
                Retry
              </Button>
            ) : undefined
          }
        />
      ) : (
        <Flex justify="center" style={{ padding: '24px 0' }}>
          <Spin />
        </Flex>
      )}
    </Card>
  )
}
