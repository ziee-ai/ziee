/**
 * About / Updates settings page (DESKTOP-ONLY).
 *
 * Shows the running version and drives the update flow:
 *   Check for updates → (if available) Download with progress →
 *   Install & restart.
 *
 * All state lives in `Stores.Updater`; this page is presentational +
 * dispatches the store actions.
 */

import {
  Alert,
  Button,
  Card,
  Descriptions,
  Progress,
  Space,
  Text,
  Paragraph,
} from '@/components/ui'
import {
  Download,
  RotateCw,
  Rocket,
} from 'lucide-react'
import { Stores } from '@/core/stores'
import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'

export function AboutPage() {
  const {
    currentVersion,
    checking,
    available,
    downloading,
    readyToInstall,
    progress,
    version,
    notes,
    error,
  } = Stores.Updater

  const upToDate = !available && !checking && !downloading && !readyToInstall

  return (
    <SettingsPageContainer title="About" subtitle="Application version and updates">
      <Card>
        <Descriptions
          column={1}
          size="sm"
          items={[
            { key: 'application', label: 'Application', children: 'Ziee' },
            {
              key: 'version',
              label: 'Version',
              children: <Text code>{currentVersion ?? '—'}</Text>,
            },
          ]}
        />

        {error && (
          <Alert
            tone="error"
            className="mt-4"
            title="Update error"
            description={error}
          />
        )}

        {available && version && (
          <Alert
            tone="info"
            className="mt-4"
            title={`Version ${version} is available`}
            description={notes ? <Paragraph className="mb-0">{notes}</Paragraph> : undefined}
          />
        )}

        {downloading && (
          <div className="mt-4">
            <Text type="secondary">Downloading update…</Text>
            <Progress
              value={Math.round(progress ?? 0)}
              tone="primary"
              aria-label="Update download progress"
            />
          </div>
        )}

        <Space className="mt-4" wrap>
          <Button
            icon={<RotateCw />}
            loading={checking}
            disabled={downloading}
            onClick={() => Stores.Updater.check()}
          >
            Check for updates
          </Button>

          {available && !readyToInstall && (
            <Button
              icon={<Download />}
              loading={downloading}
              onClick={() => Stores.Updater.download()}
            >
              Download update
            </Button>
          )}

          {readyToInstall && (
            <Button
              icon={<Rocket />}
              onClick={() => Stores.Updater.install()}
            >
              Install &amp; restart
            </Button>
          )}
        </Space>

        {upToDate && currentVersion && (
          <Paragraph type="secondary" className="mt-4 mb-0">
            You're on the latest version.
          </Paragraph>
        )}
      </Card>
    </SettingsPageContainer>
  )
}
