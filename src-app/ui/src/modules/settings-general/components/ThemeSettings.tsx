import { Card, Flex, Form, Select, Typography } from 'antd'
import { useEffect } from 'react'
import { Stores } from '@/core/stores'

const { Text } = Typography

export function ThemeSettings() {
  const { themePreference } = Stores.ConfigClient
  const [form] = Form.useForm()

  useEffect(() => {
    form.setFieldsValue({ theme: themePreference })
  }, [themePreference, form])

  const handleFormChange = (changedValues: any) => {
    if ('theme' in changedValues) {
      Stores.ConfigClient.setThemePreference(changedValues.theme)
    }
  }

  return (
    <Card title="Appearance">
      <Form
        name="theme-form"
        form={form}
        onValuesChange={handleFormChange}
        initialValues={{ theme: themePreference }}
      >
        <Flex
          justify="space-between"
          align="flex-start"
          wrap
          gap="small"
          className="min-w-0"
        >
          <div className="flex-1 min-w-80">
            <Text strong>Theme</Text>
            <div>
              <Text type="secondary">
                Choose your preferred theme or match the OS theme.
              </Text>
            </div>
          </div>
          <div className="flex-shrink-0">
            <Form.Item name="theme" style={{ margin: 0 }}>
              <Select
                aria-label="Theme"
                style={{ minWidth: 120 }}
                options={[
                  { value: 'light', label: 'Light' },
                  { value: 'dark', label: 'Dark' },
                  { value: 'system', label: 'System' },
                ]}
              />
            </Form.Item>
          </div>
        </Flex>
      </Form>
    </Card>
  )
}
