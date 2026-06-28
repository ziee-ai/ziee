/**
 * Admin "About" settings page (web UI): server version + update status.
 * Notification only — updating is a manual operator action (install.sh).
 */

import { Alert, Button, Card, Descriptions, Spin, Tag, Tooltip, Typography } from 'antd'
import { ReloadOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'
import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'

const { Text, Paragraph, Link } = Typography

const UPGRADE_COMMAND =
  'curl -fsSL https://github.com/phibya/ziee-chat-new/releases/latest/download/install.sh | sh'

export default function AboutSettings() {
  const {
    currentVersion,
    latestVersion,
    updateAvailable,
    releaseUrl,
    notes,
    enabled,
    checkedAt,
    loading,
    error,
  } = Stores.ServerUpdate

  if (loading && currentVersion == null) {
    return (
      <SettingsPageContainer title="About" subtitle="Server version and updates">
        <div className="flex justify-center py-12">
          <Spin />
        </div>
      </SettingsPageContainer>
    )
  }

  return (
    <SettingsPageContainer title="About" subtitle="Server version and updates">
      <Card>
        <Descriptions column={1} size="small" colon>
          <Descriptions.Item label="Application">Ziee server</Descriptions.Item>
          <Descriptions.Item label="Current version">
            <Text code>{currentVersion ?? '—'}</Text>
          </Descriptions.Item>
          <Descriptions.Item label="Latest version">
            {latestVersion ? (
              <>
                <Text code>{latestVersion}</Text>{' '}
                {updateAvailable ? (
                  <Tag color="blue">update available</Tag>
                ) : (
                  <Tag color="green">up to date</Tag>
                )}
              </>
            ) : (
              <Text type="secondary">{enabled ? 'not checked yet' : '—'}</Text>
            )}
          </Descriptions.Item>
          {checkedAt && (
            <Descriptions.Item label="Last checked">
              <Text type="secondary">{new Date(checkedAt).toLocaleString()}</Text>
            </Descriptions.Item>
          )}
        </Descriptions>

        {!enabled && (
          <Alert
            type="info"
            showIcon
            style={{ marginTop: 16 }}
            title="Update checks are disabled by operator config"
            description="Set update_check.enabled: true to receive update notifications."
          />
        )}

        {error && (
          <Alert type="error" showIcon style={{ marginTop: 16 }} title={error} />
        )}

        {updateAvailable && (
          <div style={{ marginTop: 16 }}>
            <Paragraph>
              A newer version is available.{' '}
              {releaseUrl && (
                <Link href={releaseUrl} target="_blank" rel="noreferrer">
                  Release notes
                </Link>
              )}
            </Paragraph>
            {notes && (
              <Paragraph
                type="secondary"
                style={{ whiteSpace: 'pre-wrap', marginBottom: 12 }}
              >
                {notes}
              </Paragraph>
            )}
            <Paragraph type="secondary" style={{ marginBottom: 4 }}>
              To update, run on the server host:
            </Paragraph>
            <Paragraph
              copyable={{ text: UPGRADE_COMMAND }}
              code
              style={{ whiteSpace: 'pre-wrap' }}
            >
              {UPGRADE_COMMAND}
            </Paragraph>
          </div>
        )}

        <Tooltip title="Reloads the most recent update check. The server checks GitHub on its own schedule; this does not force an immediate check.">
          <Button
            style={{ marginTop: 8 }}
            icon={<ReloadOutlined />}
            loading={loading}
            onClick={() => Stores.ServerUpdate.loadStatus()}
          >
            Reload status
          </Button>
        </Tooltip>
      </Card>
    </SettingsPageContainer>
  )
}
