import { useMemo } from 'react'

import {
  Flex,
  Input,
  InputNumber,
  Segmented,
  Select,
  Text,
} from '@/components/ui'

export interface ScheduleValue {
  schedule_kind: 'once' | 'recurring'
  run_at?: string // ISO, for 'once'
  cron_expr?: string // 5-field POSIX cron, for 'recurring'
  timezone: string
}

interface Props {
  value: ScheduleValue
  onChange: (v: ScheduleValue) => void
}

const DOW = [
  { label: 'Sunday', value: '0' },
  { label: 'Monday', value: '1' },
  { label: 'Tuesday', value: '2' },
  { label: 'Wednesday', value: '3' },
  { label: 'Thursday', value: '4' },
  { label: 'Friday', value: '5' },
  { label: 'Saturday', value: '6' },
]

/** Parse a preset out of a cron string (best-effort; falls back to 'custom'). */
function presetOf(
  cron: string | undefined,
): 'daily' | 'weekly' | 'monthly' | 'custom' {
  if (!cron) return 'daily'
  const p = cron.trim().split(/\s+/)
  if (p.length !== 5) return 'custom'
  const [, , dom, mon, dow] = p
  if (dom === '*' && mon === '*' && dow === '*') return 'daily'
  if (dom === '*' && mon === '*' && dow !== '*') return 'weekly'
  if (dom !== '*' && mon === '*' && dow === '*') return 'monthly'
  return 'custom'
}

function timeOf(cron: string | undefined): { min: number; hour: number } {
  const p = (cron ?? '0 9 * * *').trim().split(/\s+/)
  return { min: Number(p[0]) || 0, hour: Number(p[1]) || 0 }
}

/**
 * Preset-first recurring-schedule builder (DEC-3): Daily / Weekly / Monthly
 * emit a POSIX cron under the hood, with a raw-cron escape hatch. `once` uses a
 * datetime. The cron value is the stored source of truth.
 */
export function ScheduleBuilder({ value, onChange }: Props) {
  const preset = presetOf(value.cron_expr)
  const { min, hour } = timeOf(value.cron_expr)
  const dow = (value.cron_expr ?? '0 9 * * 1').trim().split(/\s+/)[4] || '1'
  const dom = (value.cron_expr ?? '0 9 1 * *').trim().split(/\s+/)[2] || '1'

  const timeStr = useMemo(
    () => `${String(hour).padStart(2, '0')}:${String(min).padStart(2, '0')}`,
    [hour, min],
  )

  const emitCron = (
    nextPreset: string,
    h: number,
    m: number,
    nextDow: string,
    nextDom: string,
  ) => {
    let cron: string
    switch (nextPreset) {
      case 'weekly':
        cron = `${m} ${h} * * ${nextDow}`
        break
      case 'monthly':
        cron = `${m} ${h} ${nextDom} * *`
        break
      case 'custom':
        cron = value.cron_expr ?? `${m} ${h} * * *`
        break
      default:
        cron = `${m} ${h} * * *`
    }
    onChange({ ...value, schedule_kind: 'recurring', cron_expr: cron })
  }

  return (
    <Flex className="flex-col gap-3">
      <Segmented
        data-testid="schedule-kind"
        value={value.schedule_kind}
        onChange={v =>
          onChange({
            ...value,
            schedule_kind: v as 'once' | 'recurring',
            cron_expr:
              v === 'recurring' ? (value.cron_expr ?? '0 9 * * *') : undefined,
            run_at: v === 'once' ? (value.run_at ?? '') : undefined,
          })
        }
        options={[
          { label: 'Once', value: 'once' },
          { label: 'Recurring', value: 'recurring' },
        ]}
      />

      {value.schedule_kind === 'once' ? (
        <label className="flex flex-col gap-1">
          <Text className="text-sm">Run at</Text>
          <input
            data-testid="schedule-run-at"
            type="datetime-local"
            className="rounded-md border bg-background px-2 py-1"
            value={value.run_at ? value.run_at.slice(0, 16) : ''}
            onChange={e =>
              onChange({
                ...value,
                run_at: new Date(e.target.value).toISOString(),
              })
            }
          />
        </label>
      ) : (
        <Flex className="flex-col gap-2">
          <Select
            data-testid="schedule-preset"
            value={preset}
            onChange={v => emitCron(v as string, hour, min, dow, dom)}
            options={[
              { label: 'Daily', value: 'daily' },
              { label: 'Weekly', value: 'weekly' },
              { label: 'Monthly', value: 'monthly' },
              { label: 'Custom (cron)', value: 'custom' },
            ]}
          />

          {preset !== 'custom' && (
            <label className="flex flex-col gap-1">
              <Text className="text-sm">Time</Text>
              <input
                data-testid="schedule-time"
                type="time"
                className="w-40 rounded-md border bg-background px-2 py-1"
                value={timeStr}
                onChange={e => {
                  const [h, m] = e.target.value.split(':').map(Number)
                  emitCron(preset, h || 0, m || 0, dow, dom)
                }}
              />
            </label>
          )}

          {preset === 'weekly' && (
            <Select
              data-testid="schedule-dow"
              value={dow}
              onChange={v => emitCron('weekly', hour, min, v as string, dom)}
              options={DOW}
            />
          )}

          {preset === 'monthly' && (
            <label className="flex flex-col gap-1">
              <Text className="text-sm">Day of month</Text>
              <InputNumber
                data-testid="schedule-dom"
                min={1}
                max={28}
                value={Number(dom)}
                onChange={v =>
                  emitCron('monthly', hour, min, dow, String(v ?? 1))
                }
              />
            </label>
          )}

          {preset === 'custom' && (
            <Input
              data-testid="schedule-cron"
              placeholder="min hour dom mon dow (e.g. 0 9 * * 1)"
              value={value.cron_expr ?? ''}
              onChange={e => onChange({ ...value, cron_expr: e.target.value })}
            />
          )}
        </Flex>
      )}

      <Input
        data-testid="schedule-timezone"
        placeholder="Timezone (IANA, e.g. America/New_York)"
        value={value.timezone}
        onChange={e => onChange({ ...value, timezone: e.target.value })}
      />
    </Flex>
  )
}
