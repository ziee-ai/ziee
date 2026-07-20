import { Button, message } from '@ziee/kit'
import { MdOutlineMonitorHeart } from 'react-icons/md'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/permissions'
import { Seam } from '@ziee/framework/overrides'

/**
 * "Monitor" button shown in the Hardware settings title bar.
 *
 * Core (web): opens `/hardware-monitor` as a browser popup window.
 *
 * The desktop opens a native Tauri window instead. Rather than fork this whole
 * file, the button element is wrapped in a `<Seam>`: the desktop build registers
 * `hardware.monitor-button` (see
 * `desktop/ui/src/modules/desktop-base/overrides/hardware-monitor.tsx`) and the seam swaps in the
 * native-window variant. The permission gate below stays SHARED — it lives once,
 * here, so the desktop variant carries only the ~10 lines that actually differ.
 *
 * Permission-gated: returns null when the viewer lacks `hardware::monitor`; the
 * seam is inside the gate, so the desktop override also only renders when
 * permitted.
 */
declare module '@ziee/framework/overrides' {
  interface UIOverrides {
    // Override takes no props — the shared permission gate + label/icon are
    // supplied by core; only the click behavior (native window) differs.
    'hardware.monitor-button': Record<string, never>
  }
}

export function HardwareMonitorButton() {
  const canMonitor = usePermission(Permissions.HardwareMonitor)

  if (!canMonitor) return null

  const handleClick = () => {
    try {
      const popup = window.open(
        window.location.origin + '/hardware-monitor',
        'hardware-monitor', // Same name → focuses existing popup
        'noopener=yes,noreferrer=yes,width=800,height=600,scrollbars=yes,resizable=yes,menubar=no,toolbar=no',
      )
      if (popup) {
        popup.focus()
      } else {
        message.error('Please allow popups for this website')
      }
    } catch (error) {
      console.error('Error opening hardware monitor:', error)
      message.error('Failed to open hardware monitor')
    }
  }

  return (
    <Seam id="hardware.monitor-button">
      <Button data-testid="hardware-monitor-btn" icon={<MdOutlineMonitorHeart />} onClick={handleClick}>
        Monitor
      </Button>
    </Seam>
  )
}
