/**
 * DELIBERATE DIVERGENCE from core's HardwareMonitorButton.
 *
 * Core opens `/hardware-monitor` as a browser popup. On the
 * Tauri desktop we open a real OS window via `WebviewWindow`
 * instead — feels native (own taskbar/dock entry, traffic-light
 * controls, resizable, persists across app focus changes).
 *
 * Resolved by `localOverridePlugin`: HardwareSettings imports
 * `@/modules/hardware/HardwareMonitorButton` and the plugin
 * intercepts that for the desktop bundle to land here.
 *
 * Singleton: same `label` ('hardware-monitor') is reused on every
 * click. If a window with that label already exists, focus it
 * instead of opening a duplicate.
 */

import { Button, message } from '@/components/ui'
import { MdOutlineMonitorHeart } from 'react-icons/md'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'
import { WebviewWindow } from '@tauri-apps/api/webviewWindow'

const WINDOW_LABEL = 'hardware-monitor'

export function HardwareMonitorButton() {
  const canMonitor = usePermission(Permissions.HardwareMonitor)

  if (!canMonitor) return null

  const handleClick = async () => {
    try {
      // If the window already exists, just focus it.
      const existing = await WebviewWindow.getByLabel(WINDOW_LABEL)
      if (existing) {
        await existing.setFocus()
        await existing.unminimize()
        return
      }

      // Open a new native Tauri window. The same Vite dev server +
      // bundled SPA serve `/hardware-monitor` route just like in
      // web — the route handler decides what to render.
      const win = new WebviewWindow(WINDOW_LABEL, {
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
    <Button icon={<MdOutlineMonitorHeart />} onClick={handleClick}>
      Monitor
    </Button>
  )
}
