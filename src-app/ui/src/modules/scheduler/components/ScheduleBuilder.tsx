import { useEffect, useState } from 'react'

import {
  Button,
  Flex,
  Input,
  InputNumber,
  Segmented,
  Select,
  Text,
} from '@/components/ui'
import { cn } from '@/lib/utils'

import { buildWeeklyDow, isDowList } from './scheduleCron'

export interface ScheduleValue {
  schedule_kind: 'once' | 'recurring'
  run_at?: string // ISO (UTC), for 'once'
  cron_expr?: string // 5-field POSIX cron, for 'recurring'
  timezone: string
}

interface Props {
  value: ScheduleValue
  onChange: (v: ScheduleValue) => void
}

type Preset = 'daily' | 'weekly' | 'monthly' | 'custom'

// Weekly toggle order: Mon → Sun (cron day-of-week: 0=Sun … 6=Sat).
const WEEK = [
  { label: 'Mon', full: 'Monday', value: '1' },
  { label: 'Tue', full: 'Tuesday', value: '2' },
  { label: 'Wed', full: 'Wednesday', value: '3' },
  { label: 'Thu', full: 'Thursday', value: '4' },
  { label: 'Fri', full: 'Friday', value: '5' },
  { label: 'Sat', full: 'Saturday', value: '6' },
  { label: 'Sun', full: 'Sunday', value: '0' },
]

/** Classify a cron into a preset (best-effort; falls back to 'custom'). */
function presetOf(cron: string | undefined): Preset {
  if (!cron) return 'daily'
  const p = cron.trim().split(/\s+/)
  if (p.length !== 5) return 'custom'
  const [min, hour, dom, mon, dow] = p
  const numeric = (s: string) => /^\d+$/.test(s)
  if (!numeric(min) || !numeric(hour)) return 'custom'
  if (dom === '*' && mon === '*' && dow === '*') return 'daily'
  if (dom === '*' && mon === '*' && isDowList(dow)) return 'weekly'
  if (numeric(dom) && mon === '*' && dow === '*') return 'monthly'
  return 'custom'
}

function timeOf(cron: string | undefined): { min: number; hour: number } {
  const p = (cron ?? '0 9 * * *').trim().split(/\s+/)
  return { min: Number(p[0]) || 0, hour: Number(p[1]) || 0 }
}

/** A cron field, replacing a `*` with a real default (so switching presets works). */
function fieldOr(cron: string | undefined, idx: number, dflt: string): string {
  const v = (cron ?? '').trim().split(/\s+/)[idx]
  return v && v !== '*' && /^\d+$/.test(v) ? v : dflt
}

/** UTC ISO → the `YYYY-MM-DDTHH:mm` LOCAL wall-clock a datetime-local input wants. */
function toLocalInput(utcIso: string | undefined): string {
  if (!utcIso) return ''
  const d = new Date(utcIso)
  if (Number.isNaN(d.getTime())) return ''
  return new Date(d.getTime() - d.getTimezoneOffset() * 60000).toISOString().slice(0, 16)
}

/**
 * Preset-first recurring-schedule builder (DEC-3): Daily / Weekly / Monthly emit
 * a POSIX cron under the hood, with a raw-cron escape hatch. `once` uses a
 * datetime. The cron value is the stored source of truth; the selected preset is
 * LOCAL state (a cron like `0 9 * * *` is legitimately "daily", so we can't
 * re-derive 'custom' from the cron — the user's choice must stick).
 */
export function ScheduleBuilder({ value, onChange }: Props) {
  const [preset, setPreset] = useState<Preset>(() => presetOf(value.cron_expr))

  // Re-sync the preset when the seeded schedule identity changes (edit-open).
  // (The drawer uses destroyOnHidden, so this mostly matters for a live swap.)
  useEffect(() => {
    if (value.schedule_kind === 'recurring') setPreset(presetOf(value.cron_expr))
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [value.schedule_kind])

  const { min, hour } = timeOf(value.cron_expr)
  // Day-of-week is a comma list for weekly (ITEM-12): `1,3,5`. Default Monday.
  const dowRaw = (value.cron_expr ?? '').trim().split(/\s+/)[4]
  const dow = dowRaw && isDowList(dowRaw) ? dowRaw : '1'
  const selectedDays = new Set(dow.split(',').filter(Boolean))
  const dom = fieldOr(value.cron_expr, 2, '1')

  // Toggle a single day in/out of the weekly set, keeping ≥1 day selected so
  // the emitted cron day-of-week field never goes empty/invalid. Emits a SORTED
  // comma list (e.g. Mon+Wed+Fri → `1,3,5`).
  const toggleDay = (dayVal: string) => {
    const days = new Set(selectedDays)
    if (days.has(dayVal)) {
      if (days.size === 1) return
      days.delete(dayVal)
    } else {
      days.add(dayVal)
    }
    emitCron('weekly', hour, min, buildWeeklyDow(days), dom)
  }
  const timeStr = `${String(hour).padStart(2, '0')}:${String(min).padStart(2, '0')}`

  const emitCron = (nextPreset: Preset, h: number, m: number, nextDow: string, nextDom: string) => {
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

  const choosePreset = (p: Preset) => {
    setPreset(p)
    emitCron(p, hour, min, dow, dom)
  }

  return (
    <Flex className="flex-col gap-3">
      <Segmented
        data-standalone-control
        data-testid="schedule-kind"
        value={value.schedule_kind}
        onChange={v =>
          onChange({
            ...value,
            schedule_kind: v as 'once' | 'recurring',
            cron_expr: v === 'recurring' ? (value.cron_expr ?? '0 9 * * *') : undefined,
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
          <Input
            data-testid="schedule-run-at"
            type="datetime-local"
            value={toLocalInput(value.run_at)}
            onChange={e => {
              const raw = e.target.value
              if (!raw) {
                onChange({ ...value, run_at: '' })
                return
              }
              const d = new Date(raw)
              onChange({
                ...value,
                run_at: Number.isNaN(d.getTime()) ? '' : d.toISOString(),
              })
            }}
          />
        </label>
      ) : (
        <Flex className="flex-col gap-2">
          <Select
            data-standalone-control
            data-testid="schedule-preset"
            value={preset}
            onChange={v => choosePreset(v as Preset)}
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
              <Input
                data-testid="schedule-time"
                type="time"
                className="w-40"
                value={timeStr}
                onChange={e => {
                  const [h, m] = e.target.value.split(':').map(Number)
                  emitCron(preset, h || 0, m || 0, dow, dom)
                }}
              />
            </label>
          )}

          {preset === 'weekly' && (
            <Flex
              className="flex-wrap gap-1"
              data-testid="schedule-dow"
              role="group"
              aria-label="Days of week"
            >
              {WEEK.map(d => {
                const active = selectedDays.has(d.value)
                return (
                  <Button
                    key={d.value}
                    type="button"
                    data-standalone-control
                    data-testid={`schedule-dow-${d.value}`}
                    variant={active ? 'default' : 'outline'}
                    aria-pressed={active}
                    aria-label={d.full}
                    onClick={() => toggleDay(d.value)}
                    className={cn('w-14 px-0', active && 'font-semibold')}
                  >
                    {d.label}
                  </Button>
                )
              })}
            </Flex>
          )}

          {preset === 'monthly' && (
            <label className="flex flex-col gap-1">
              <Text className="text-sm">Day of month</Text>
              <InputNumber
                data-testid="schedule-dom"
                min={1}
                max={28}
                value={Number(dom)}
                onChange={v => emitCron('monthly', hour, min, dow, String(v ?? 1))}
              />
            </label>
          )}

          {preset === 'custom' && (
            <Input
              data-testid="schedule-cron"
              aria-label="Cron expression"
              placeholder="min hour dom mon dow (e.g. 0 9 * * 1)"
              value={value.cron_expr ?? ''}
              onChange={e => onChange({ ...value, cron_expr: e.target.value })}
            />
          )}
        </Flex>
      )}

      {/* ITEM-2 (FB-3): the timezone is auto-detected from the client — never an
          input the user must fill. Shown read-only so the schedule is transparent
          about which zone its times are in. */}
      <Text className="text-muted-foreground text-xs" data-testid="schedule-timezone-note">
        Times are in your timezone: {value.timezone}
      </Text>
    </Flex>
  )
}
