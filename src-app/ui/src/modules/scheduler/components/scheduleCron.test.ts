import assert from 'node:assert/strict'
import { test } from 'node:test'

import { buildWeeklyDow, humanizeCron } from './scheduleCron.ts'

// TEST-22 (ITEM-12) — the weekly multi-day cron emission + list-page humanizer.

test('selecting {Mon,Wed,Fri} emits the sorted comma dow 1,3,5', () => {
  // Mon=1, Wed=3, Fri=5. Order-insensitive input, sorted numeric output.
  assert.equal(buildWeeklyDow(['3', '1', '5']), '1,3,5')
  // The emitted recurring cron the ScheduleBuilder produces at 09:00.
  assert.equal(`0 9 * * ${buildWeeklyDow(['3', '1', '5'])}`, '0 9 * * 1,3,5')
})

test('buildWeeklyDow collapses duplicates and ignores non-numeric entries', () => {
  assert.equal(buildWeeklyDow(['1', '1', '3']), '1,3')
  assert.equal(buildWeeklyDow([1, 3, 5]), '1,3,5')
})

test('humanizeCron round-trips a multi-day weekly expression', () => {
  assert.equal(
    humanizeCron('0 9 * * 1,3,5'),
    'Weekly on Mon, Wed, Fri at 09:00',
  )
})

test('humanizeCron classifies daily / single-day-weekly / monthly', () => {
  assert.equal(humanizeCron('0 9 * * *'), 'Daily at 09:00')
  assert.equal(humanizeCron('30 14 * * 1'), 'Weekly on Mon at 14:30')
  assert.equal(humanizeCron('0 8 15 * *'), 'Monthly on day 15 at 08:00')
})

test('humanizeCron normalizes cron dow 7 to Sunday (never undefined)', () => {
  assert.equal(humanizeCron('0 9 * * 7'), 'Weekly on Sun at 09:00')
  // 0 is also Sunday; both render "Sun".
  assert.equal(humanizeCron('0 9 * * 0'), 'Weekly on Sun at 09:00')
})

test('humanizeCron falls back to `Cron: <expr>` for an unclassifiable expression', () => {
  // Non-numeric minute → not a recognized shape.
  assert.equal(humanizeCron('*/5 * * * *'), 'Cron: */5 * * * *')
  // Wrong field count.
  assert.equal(humanizeCron('0 9 * *'), 'Cron: 0 9 * *')
  // A range in the dow field isn't a comma-list of single digits.
  assert.equal(humanizeCron('0 9 * * 1-5'), 'Cron: 0 9 * * 1-5')
})
