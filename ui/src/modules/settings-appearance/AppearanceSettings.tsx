import { Card, Typography } from 'antd'

const { Text } = Typography

export default function AppearanceSettings() {
  return (
    <div className="h-full overflow-y-auto p-6">
      <Card title="Appearance Settings">
        <Text type="secondary">Appearance settings (theme, language) will be displayed here</Text>
      </Card>
    </div>
  )
}
