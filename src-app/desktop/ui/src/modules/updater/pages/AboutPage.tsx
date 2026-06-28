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
  Typography,
} from 'antd'
import {
  DownloadOutlined,
  ReloadOutlined,
  RocketOutlined,
} from '@ant-design/icons'
import { Stores } from '@/core/stores'
import { SettingsPageContainer } from '@ziee/ui-core/modules/settings/components/SettingsPageContainer'

const { Text, Paragraph } = Typography

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
        <Descriptions column={1} size="small" colon>
          <Descriptions.Item label="Application">Ziee</Descriptions.Item>
          <Descriptions.Item label="Version">
            <Text code>{currentVersion ?? '—'}</Text>
          </Descriptions.Item>
        </Descriptions>

        {error && (
          <Alert
            type="error"
            showIcon
            style={{ marginTop: 16 }}
            message="Update error"
            description={error}
          />
        )}

        {available && version && (
          <Alert
            type="info"
            showIcon
            style={{ marginTop: 16 }}
            message={`Version ${version} is available`}
            description={notes ? <Paragraph style={{ marginBottom: 0 }}>{notes}</Paragraph> : undefined}
          />
        )}

        {downloading && (
          <div style={{ marginTop: 16 }}>
            <Text type="secondary">Downloading update…</Text>
            <Progress percent={Math.round(progress ?? 0)} status="active" />
          </div>
        )}

        <Space style={{ marginTop: 16 }} wrap>
          <Button
            icon={<ReloadOutlined />}
            loading={checking}
            disabled={downloading}
            onClick={() => Stores.Updater.check()}
          >
            Check for updates
          </Button>

          {available && !readyToInstall && (
            <Button
              type="primary"
              icon={<DownloadOutlined />}
              loading={downloading}
              onClick={() => Stores.Updater.download()}
            >
              Download update
            </Button>
          )}

          {readyToInstall && (
            <Button
              type="primary"
              icon={<RocketOutlined />}
              onClick={() => Stores.Updater.install()}
            >
              Install &amp; restart
            </Button>
          )}
        </Space>

        {upToDate && currentVersion && (
          <Paragraph type="secondary" style={{ marginTop: 16, marginBottom: 0 }}>
            You're on the latest version.
          </Paragraph>
        )}
      </Card>
    </SettingsPageContainer>
  )
}
