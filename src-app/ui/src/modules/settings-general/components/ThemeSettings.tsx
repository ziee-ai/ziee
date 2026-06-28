import { Card, Flex, Select, Text } from '@/components/ui'
import { Stores } from '@/core/stores'
import type { ThemePreference } from '@/modules/config-client/ConfigClient.store'

export function ThemeSettings() {
  const { themePreference } = Stores.ConfigClient

  const handleChange = (value: string) => {
    Stores.ConfigClient.setThemePreference(value as ThemePreference)
  }

  return (
    <Card title="Appearance" data-testid="settingsgen-appearance-card">
      <Flex
        justify="between"
        align="start"
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
          <Select
            data-testid="settingsgen-theme-select"
            aria-label="Theme"
            className="min-w-[120px]"
            value={themePreference}
            onChange={handleChange}
            options={[
              { value: 'light', label: 'Light' },
              { value: 'dark', label: 'Dark' },
              { value: 'system', label: 'System' },
            ]}
          />
        </div>
      </Flex>
    </Card>
  )
}
