/**
 * Admin "About" settings page (web UI): server version + update status.
 * Notification only — updating is a manual operator action (install.sh).
 */

import { ReloadOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'
import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'
import {
  Alert,
  Button,
  Card,
  Descriptions,
  Tag,
  Text,
  Paragraph,
  Link,
} from '@/components/ui'

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

  return (
    <SettingsPageContainer title="About" subtitle="Server version and updates">
      <Card>
        <Descriptions
          column={1}
          size="sm"
          items={[
            {
              key: 'application',
              label: 'Application',
              children: 'Ziee server',
            },
            {
              key: 'current-version',
              label: 'Current version',
              children: <Text code>{currentVersion ?? '—'}</Text>,
            },
            {
              key: 'latest-version',
              label: 'Latest version',
              children: latestVersion ? (
                <>
                  <Text code>{latestVersion}</Text>{' '}
                  {updateAvailable ? (
                    <Tag tone="info">update available</Tag>
                  ) : (
                    <Tag tone="success">up to date</Tag>
                  )}
                </>
              ) : (
                <Text type="secondary">{enabled ? 'not checked yet' : '—'}</Text>
              ),
            },
            ...(checkedAt
              ? [
                  {
                    key: 'last-checked',
                    label: 'Last checked',
                    children: (
                      <Text type="secondary">
                        {new Date(checkedAt).toLocaleString()}
                      </Text>
                    ),
                  },
                ]
              : []),
          ]}
        />

        {!enabled && (
          <Alert
            tone="info"
            className="mt-4"
            title="Update checks are disabled by operator config"
            description="Set update_check.enabled: true to receive update notifications."
          />
        )}

        {error && (
          <Alert tone="error" className="mt-4" title={error} />
        )}

        {updateAvailable && (
          <div className="mt-4">
            <Paragraph>
              A newer version is available.{' '}
              {releaseUrl && (
                <Link href={releaseUrl} target="_blank" rel="noreferrer">
                  Release notes
                </Link>
              )}
            </Paragraph>
            {notes && (
              <Paragraph type="secondary" className="whitespace-pre-wrap mb-3">
                {notes}
              </Paragraph>
            )}
            <Paragraph type="secondary" className="mb-1">
              To update, run on the server host:
            </Paragraph>
            <Paragraph
              copyable={{ text: UPGRADE_COMMAND, label: 'Copy upgrade command' }}
              code
              className="whitespace-pre-wrap"
            >
              {UPGRADE_COMMAND}
            </Paragraph>
          </div>
        )}

        <Button
          className="mt-2"
          icon={<ReloadOutlined />}
          loading={loading}
          onClick={() => Stores.ServerUpdate.loadStatus()}
        >
          Refresh
        </Button>
      </Card>
    </SettingsPageContainer>
  )
}
