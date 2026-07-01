/**
 * Admin "About" settings page (web UI): server version + update status.
 * Notification only — updating is a manual operator action (install.sh).
 */

import { RotateCw } from 'lucide-react'
import { Stores } from '@/core/stores'
import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'
import {
  Alert,
  Button,
  Card,
  Descriptions,
  Spin,
  Tag,
  Text,
  Paragraph,
  Link,
  Tooltip,
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

  // Show the loading state until the first status load resolves — not
  // only while `loading` is explicitly true. The store sets `loading`
  // via an async __init__, so the very first synchronous render has
  // loading=false + currentVersion=null; without this the card would
  // flash em-dash placeholders before the fetch kicks in. An error
  // (currentVersion still null) falls through to render the card so the
  // error Alert is visible.
  if (currentVersion == null && error == null) {
    return (
      <SettingsPageContainer title="About" subtitle="Server version and updates">
        <div className="flex justify-center py-12">
          <Spin label="Loading status" />
        </div>
      </SettingsPageContainer>
    )
  }

  return (
    <SettingsPageContainer title="About" subtitle="Server version and updates">
      <Card data-testid="serverupd-about-card">
        <Descriptions
          data-testid="serverupd-about-descriptions"
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
                    <Tag variant="outline" data-testid="serverupd-update-available-tag" tone="info">update available</Tag>
                  ) : (
                    <Tag variant="outline" data-testid="serverupd-uptodate-tag" tone="success">up to date</Tag>
                  )}
                </>
              ) : (
                <Text data-testid="serverupd-not-checked" type="secondary">{enabled ? 'not checked yet' : '—'}</Text>
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
            data-testid="serverupd-disabled-alert"
            tone="info"
            className="mt-4"
            title="Update checks are disabled by operator config"
            description="Set update_check.enabled: true to receive update notifications."
          />
        )}

        {error && (
          <Alert data-testid="serverupd-error-alert" tone="error" className="mt-4" title={error} />
        )}

        {updateAvailable && (
          <div className="mt-4">
            <Paragraph>
              A newer version is available.{' '}
              {releaseUrl && (
                <Link data-testid="serverupd-release-notes-link" href={releaseUrl} target="_blank" rel="noreferrer">
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
              data-testid="serverupd-upgrade-command"
              copyable={{ text: UPGRADE_COMMAND, label: 'Copy upgrade command', testId: 'serverupd-copy-cmd-btn' }}
              code
              className="whitespace-pre-wrap"
            >
              {UPGRADE_COMMAND}
            </Paragraph>
          </div>
        )}

        <Tooltip content="Reloads the most recent update check. The server checks GitHub on its own schedule; this does not force an immediate check.">
          <Button
            data-testid="serverupd-refresh-btn"
            className="mt-2"
            icon={<RotateCw />}
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
