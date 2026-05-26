import React from 'react'
import { Alert } from 'antd'

// TODO: Implement device selection using Hardware API
// The Hardware API exists at /api/hardware but needs to be adapted
// React-test used Admin.getAvailableDevices() which doesn't exist in ziee-chat
// Need to use Hardware.info() or create a new endpoint for device enumeration
export const DeviceSelectionSection: React.FC = () => {
  return (
    <Alert
      type="info"
      title="Device Selection Coming Soon"
      description="Device selection will be implemented once the Hardware API is fully integrated."
      style={{ marginBottom: 16 }}
    />
  )
}
