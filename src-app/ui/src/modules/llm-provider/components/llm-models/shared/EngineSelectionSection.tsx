import React from 'react'
import { Alert } from 'antd'

// TODO: Implement engine selection
// React-test used a @/store import which doesn't exist in ziee's module architecture
// Need to adapt to use module stores or remove if not needed
export const EngineSelectionSection: React.FC = () => {
  return (
    <Alert
      type="info"
      title="Engine Selection Coming Soon"
      description="Engine selection will be available once store integration is complete."
      style={{ marginBottom: 16 }}
    />
  )
}
