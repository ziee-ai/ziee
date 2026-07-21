/**
 * About / Updates settings page (DESKTOP-ONLY).
 *
 * Shows the running version and drives the update flow:
 *   Check for updates → (if available) Download with progress →
 *   Install & restart.
 *
 * All state lives in `Updater`; this page is presentational +
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
} from '@ziee/kit'
import {
  Download,
  RotateCw,
  Rocket,
} from 'lucide-react'
import { SettingsPageContainer } from '@ziee/ui-core/modules/settings/components/SettingsPageContainer'
import { Updater } from '@ziee/desktop/modules/updater/stores/updater'

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
  } = Updater

  const upToDate = !available && !checking && !downloading && !readyToInstall

  return (
    <SettingsPageContainer title="About" subtitle="Application version and updates">
      <Card data-testid="desktop-updater-about-card">
        <Descriptions
          data-testid="desktop-updater-about-descriptions"
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
            data-testid="desktop-updater-about-error-alert"
            tone="error"
            className="mt-4"
            title="Update error"
            description={error}
          />
        )}

        {available && version && (
          <Alert
            data-testid="desktop-updater-about-available-alert"
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
              data-testid="desktop-updater-about-progress"
              value={Math.round(progress ?? 0)}
              tone="primary"
              aria-label="Update download progress"
            />
          </div>
        )}

        <Space className="mt-4" wrap>
          <Button
            data-testid="desktop-updater-about-check-btn"
            icon={<RotateCw />}
            loading={checking}
            disabled={downloading}
            onClick={() => Updater.check()}
          >
            Check for updates
          </Button>

          {available && !readyToInstall && (
            <Button
              data-testid="desktop-updater-about-download-btn"
              icon={<Download />}
              loading={downloading}
              onClick={() => Updater.download()}
            >
              Download update
            </Button>
          )}

          {readyToInstall && (
            <Button
              data-testid="desktop-updater-about-install-btn"
              icon={<Rocket />}
              onClick={() => Updater.install()}
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
