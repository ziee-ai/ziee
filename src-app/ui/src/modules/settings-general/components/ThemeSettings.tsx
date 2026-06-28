import { Check } from 'lucide-react'
import { Button, Card, Flex, Select, Text } from '@/components/ui'
import { Stores } from '@/core/stores'
import type { ThemePreference } from '@/modules/config-client/ConfigClient.store'
import {
  ACCENT_PRESETS,
  ACCENT_ORDER,
  type AccentPreset,
} from '@/components/ThemeProvider/accentPresets'
import { cn } from '@/lib/utils'

export function ThemeSettings() {
  const { themePreference, accentPreset } = Stores.ConfigClient

  const handleChange = (value: string) => {
    Stores.ConfigClient.setThemePreference(value as ThemePreference)
  }

  return (
    <Card title="Appearance" data-testid="settingsgen-appearance-card">
      <Flex vertical gap="middle" className="min-w-0">
        <Flex justify="between" align="start" wrap gap="small" className="min-w-0">
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

        <Flex justify="between" align="start" wrap gap="small" className="min-w-0">
          <div className="flex-1 min-w-80">
            <Text strong>Accent color</Text>
            <div>
              <Text type="secondary">
                Used for buttons, links, focus rings, and selected items.
              </Text>
            </div>
          </div>
          <Flex
            gap="small"
            wrap
            align="center"
            className="flex-shrink-0"
            data-testid="settingsgen-accent-picker"
          >
            {ACCENT_ORDER.map(id => {
              const def = ACCENT_PRESETS[id as AccentPreset]
              const selected = accentPreset === id
              return (
                <Button
                  key={id}
                  size="icon"
                  variant="ghost"
                  aria-label={`${def.label} accent`}
                  data-testid={`settingsgen-accent-${id}`}
                  onClick={() => Stores.ConfigClient.setAccentPreset(id as AccentPreset)}
                  style={{ backgroundColor: `hsl(${def.light.primary})` }}
                  className={cn(
                    // inline bg wins over ghost's hover bg, so signal hover via scale instead.
                    'size-7 rounded-full border border-border/40 transition-transform hover:scale-110',
                    selected && 'ring-2 ring-offset-2 ring-offset-background ring-foreground',
                  )}
                >
                  {selected && <Check className="size-4 text-white" aria-hidden />}
                </Button>
              )
            })}
          </Flex>
        </Flex>
      </Flex>
    </Card>
  )
}
