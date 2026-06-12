/**
 * Sidebar update card (DESKTOP-ONLY).
 *
 * Rendered in the left sider's footer (just above the user profile) via the
 * `sidebarFooter` slot. Appears only when the silent on-load check found a
 * newer version and the user hasn't dismissed it this session.
 *
 *   ┌─────────────────────────────┐
 *   │ ⬆ Update available   v0.2.0 │
 *   │ [Remind later] [Install &…] │
 *   └─────────────────────────────┘
 *
 * "Install & restart" downloads, then auto-installs + relaunches the moment
 * the bytes are ready; the buttons are replaced by a progress bar meanwhile.
 */

import { Button, Progress, Space, Typography, theme } from 'antd'
import { ArrowUpOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'

const { Text } = Typography

export function UpdateBanner() {
  const { token } = theme.useToken()
  const {
    available,
    dismissed,
    version,
    downloading,
    readyToInstall,
    progress,
    error,
  } = Stores.Updater

  if (!available || dismissed) return null

  const installing = downloading || readyToInstall

  return (
    <div style={{ padding: '4px 8px' }}>
      <div
        style={{
          border: `1px solid ${token.colorBorderSecondary}`,
          background: token.colorFillQuaternary,
          borderRadius: token.borderRadius,
          padding: 10,
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginBottom: 8 }}>
          <ArrowUpOutlined style={{ color: token.colorPrimary }} />
          <Text strong style={{ flex: 1 }}>
            Update available
          </Text>
          {version && (
            <Text type="secondary" style={{ fontSize: 12 }}>
              v{version}
            </Text>
          )}
        </div>

        {installing ? (
          <div>
            <Text type="secondary" style={{ fontSize: 12 }}>
              {readyToInstall ? 'Installing…' : 'Downloading…'}
            </Text>
            <Progress
              percent={Math.round(progress ?? 0)}
              size="small"
              status="active"
              style={{ marginBottom: 0 }}
            />
          </div>
        ) : (
          <Space size="small" style={{ width: '100%' }}>
            <Button size="small" onClick={() => Stores.Updater.remindLater()}>
              Remind later
            </Button>
            <Button
              size="small"
              type="primary"
              onClick={() => Stores.Updater.installAndRestart()}
            >
              Install &amp; restart
            </Button>
          </Space>
        )}

        {error && (
          <Text type="danger" style={{ fontSize: 12, display: 'block', marginTop: 6 }}>
            {error}
          </Text>
        )}
      </div>
    </div>
  )
}
