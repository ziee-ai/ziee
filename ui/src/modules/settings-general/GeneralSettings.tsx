import { Card, Typography } from 'antd'

const { Text } = Typography

export default function GeneralSettings() {
  return (
    <div className="h-full overflow-y-auto p-6">
      <Card title="General Settings">
        <Text type="secondary">General settings will be displayed here</Text>
      </Card>
    </div>
  )
}
