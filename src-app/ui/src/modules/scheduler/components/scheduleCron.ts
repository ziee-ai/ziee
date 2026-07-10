/**
 * Pure cron helpers for the scheduler UI (ITEM-12). Extracted from the React
 * components so the weekly day-of-week emission (`buildWeeklyDow`) and the
 * list-page summary (`humanizeCron`) are unit-testable under `node --test`
 * (a `.tsx` module can't be imported there — JSX doesn't type-strip).
 *
 * cron day-of-week: 0 = Sunday … 6 = Saturday (7 is also Sunday in POSIX crontab).
 */

export const DOW_SHORT = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat']

/** A cron day-of-week field that is a single day OR a comma list (e.g. `1,3,5`). */
export const isDowList = (s: string): boolean => /^\d(,\d)*$/.test(s)

/**
 * Build the cron day-of-week field from a set of selected day values: numeric
 * ascending, comma-joined (e.g. Mon+Wed+Fri → `1,3,5`). Duplicates are collapsed;
 * non-numeric entries are ignored.
 */
export function buildWeeklyDow(days: Iterable<string | number>): string {
  return [...new Set(Array.from(days, Number))]
    .filter(n => Number.isFinite(n))
    .sort((a, b) => a - b)
    .join(',')
}

/**
 * Render a 5-field POSIX cron into a human sentence. Classifies daily / weekly
 * (single or multi-day) / monthly; anything else falls back to `Cron: <expr>`.
 */
export function humanizeCron(cron: string): string {
  const p = cron.trim().split(/\s+/)
  if (p.length !== 5) return `Cron: ${cron}`
  const [min, hour, dom, mon, dow] = p
  const t =
    /^\d+$/.test(min) && /^\d+$/.test(hour)
      ? `${hour.padStart(2, '0')}:${min.padStart(2, '0')}`
      : null
  if (!t) return `Cron: ${cron}`
  if (dom === '*' && mon === '*' && dow === '*') return `Daily at ${t}`
  if (dom === '*' && mon === '*' && isDowList(dow)) {
    // cron dow 7 is Sunday in many dialects (same as 0) — normalize mod 7 so it
    // never indexes past DOW_SHORT (was rendering "undefined").
    const days = dow
      .split(',')
      .map(n => Number(n) % 7)
      .sort((a, b) => a - b)
      .map(n => DOW_SHORT[n])
      .join(', ')
    return `Weekly on ${days} at ${t}`
  }
  if (/^\d+$/.test(dom) && mon === '*' && dow === '*')
    return `Monthly on day ${dom} at ${t}`
  return `Cron: ${cron}`
}
