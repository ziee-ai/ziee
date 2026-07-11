/**
 * Desktop override: Hardware "Monitor" button (seam `hardware.monitor-button`).
 *
 * Each file under `overrides/` owns ONE seam and exports a `register()` that
 * `overrides/index.ts` auto-discovers + calls (synchronously, pre-render). The
 * `@/…` alias resolves to core (no desktop shadow exists for these), so the
 * seam-key declaration merging from the core component applies to
 * `registerOverride` here. The `seam` codemod scaffolds this file.
 */
import { registerOverride } from '@/core/overrides'
import { Button, message } from '@/components/ui'
import { MdOutlineMonitorHeart } from 'react-icons/md'
import { WebviewWindow } from '@tauri-apps/api/webviewWindow'

const HARDWARE_MONITOR_WINDOW_LABEL = 'hardware-monitor'

/**
 * Desktop variant of the Hardware "Monitor" button — opens a real OS window via
 * `WebviewWindow` (own taskbar/dock entry, resizable, persists across focus)
 * instead of a browser popup. Singleton: focuses the existing window if open.
 * No permission check — the shared core seam only renders inside the
 * `hardware::monitor` gate.
 */
function DesktopHardwareMonitorButton() {
  const handleClick = async () => {
    try {
      const existing = await WebviewWindow.getByLabel(
        HARDWARE_MONITOR_WINDOW_LABEL,
      )
      if (existing) {
        await existing.setFocus()
        await existing.unminimize()
        return
      }

      const win = new WebviewWindow(HARDWARE_MONITOR_WINDOW_LABEL, {
        url: '/hardware-monitor',
        title: 'Hardware Monitor',
        width: 900,
        height: 640,
        minWidth: 480,
        minHeight: 360,
        resizable: true,
        center: true,
      })

      win.once('tauri://error', (e: unknown) => {
        console.error('Hardware monitor window failed to open:', e)
        message.error('Failed to open hardware monitor window')
      })
    } catch (error) {
      console.error('Error opening hardware monitor:', error)
      message.error('Failed to open hardware monitor')
    }
  }

  return (
    <Button
      data-testid="desktop-hardware-monitor-btn"
      icon={<MdOutlineMonitorHeart />}
      onClick={handleClick}
    >
      Monitor
    </Button>
  )
}

/** Register this file's seam(s). Auto-called by `overrides/index.ts`. */
export function register(): void {
  registerOverride('hardware.monitor-button', DesktopHardwareMonitorButton)
}
