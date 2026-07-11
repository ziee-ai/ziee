/**
 * Desktop UI Override registrations.
 *
 * Every element-level desktop override (a `<Seam>` declared in a core web
 * component) is registered here. `registerDesktopOverrides()` is invoked
 * SYNCHRONOUSLY from `main.tsx` before `ReactDOM.render`, so an override is in
 * the registry before the core component that reads its seam first renders
 * (same pre-render window as `Stores.AppMode.setMultiUserMode(false)`).
 *
 * Imports use the `@/…` alias (resolved to core for typecheck, and — since no
 * desktop shadow exists for these — to core at runtime too), so the seam-key
 * declaration merging from the core files applies to `registerOverride` here.
 *
 * Adding an override: declare the seam in the core component (a `<Seam>` +
 * `declare module '@/core/overrides'`), then add ONE `registerOverride(...)`
 * line below. The `seam` codemod scaffolds both sides.
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

/** Register every desktop UI override. Call once, synchronously, pre-render. */
export function registerDesktopOverrides(): void {
  registerOverride('hardware.monitor-button', DesktopHardwareMonitorButton)
}
