import { Button, message } from '@/components/ui'
import { MdOutlineMonitorHeart } from 'react-icons/md'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'

/**
 * "Monitor" button shown in the Hardware settings title bar.
 *
 * Core (web): opens `/hardware-monitor` as a browser popup window.
 *
 * Desktop overrides this file via `localOverridePlugin` to open a
 * native Tauri window instead — the desktop bundle imports
 * `desktop/ui/src/modules/hardware/HardwareMonitorButton.tsx` for
 * any `@/modules/hardware/HardwareMonitorButton` reference, so
 * HardwareSettings keeps its single call site and the desktop's
 * window machinery stays out of core.
 *
 * Permission-gated: returns null when the viewer lacks
 * `hardware::monitor` (same condition as the previous inline check
 * in HardwareSettings).
 */
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
    <Button data-testid="hardware-monitor-btn" icon={<MdOutlineMonitorHeart />} onClick={handleClick}>
      Monitor
    </Button>
  )
}
