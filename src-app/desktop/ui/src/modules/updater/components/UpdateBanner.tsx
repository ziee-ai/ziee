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

import { Button, Progress, Space, Text } from '@ziee/kit'
import { ArrowUp } from 'lucide-react'
import { Stores } from '@/core/stores'

export function UpdateBanner() {
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
    <div className="px-2 py-1">
      <div className="border border-border bg-muted/40 rounded-md p-2.5">
        <div className="flex items-center gap-1.5 mb-2">
          <ArrowUp className="size-4 text-primary" aria-hidden />
          <Text strong className="flex-1">
            Update available
          </Text>
          {version && (
            <Text type="secondary" className="text-xs">
              v{version}
            </Text>
          )}
        </div>

        {installing ? (
          <div>
            <Text type="secondary" className="text-xs">
              {readyToInstall ? 'Installing…' : 'Downloading…'}
            </Text>
            <Progress
              data-testid="desktop-updater-banner-progress"
              value={Math.round(progress ?? 0)}
              size="sm"
              tone="primary"
              aria-label="Update download progress"
            />
          </div>
        ) : (
          <Space size="small" className="w-full">
            <Button data-testid="desktop-updater-banner-remind-btn" size="default" onClick={() => Stores.Updater.remindLater()}>
              Remind later
            </Button>
            <Button
              data-testid="desktop-updater-banner-install-btn"
              size="default"
              onClick={() => Stores.Updater.installAndRestart()}
            >
              Install &amp; restart
            </Button>
          </Space>
        )}

        {error && (
          <Text type="danger" className="text-xs block mt-1.5">
            {error}
          </Text>
        )}
      </div>
    </div>
  )
}
