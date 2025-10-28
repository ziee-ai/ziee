import { Card, Typography } from 'antd'

const { Text } = Typography

export default function PrivacySettings() {
  return (
    <div className="h-full overflow-y-auto p-6">
      <Card title="Privacy Settings">
        <Text type="secondary">Privacy settings will be displayed here</Text>
      </Card>
    </div>
  )
}
