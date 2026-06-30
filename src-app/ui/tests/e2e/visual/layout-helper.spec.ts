/**
 * META-TEST for the Layer-A detector (`assertLayoutSane`).
 *
 * The blind audit found the spacing/radius check was mathematically DEAD
 * (tolerance >= grid/2 → every value "on-scale") and shipped green — a unit test
 * of the detector would have caught it. This spec asserts each invariant actually
 * FIRES on a known-bad DOM snippet and PASSES on a known-good one, so the
 * detector can't silently rot (and the dead-tolerance class of bug is impossible
 * to reintroduce unnoticed).
 *
 * Self-contained: builds synthetic DOM with `page.setContent()` (no gallery, no
 * baseline). Uses `collect: true` so we inspect the violations directly.
 */
import { expect, test } from '@playwright/test'
import {
  type LayoutCheck,
  type LayoutViolation,
  assertLayoutSane,
} from '../helpers/layout'
import type { Page } from '@playwright/test'

/** Collect violations for one scoped fixture (only the named check enabled). */
async function violationsFor(
  page: Page,
  testid: string,
  only: LayoutCheck,
): Promise<LayoutViolation[]> {
  const checks: Partial<Record<LayoutCheck, boolean>> = {
    horizontalScroll: false,
    childOverflow: false,
    siblingOverlap: false,
    spacingScale: false,
    buttonWidth: false,
    touchTarget: false,
    textTruncation: false,
  }
  checks[only] = true
  const v = await assertLayoutSane(page.getByTestId(testid), {
    checks,
    collect: true,
  })
  return v.filter(x => x.check === only)
}

const has = (v: LayoutViolation[], check: LayoutCheck) =>
  v.some(x => x.check === check)

test('spacingScale fires on off-grid padding, not on on-grid', async ({
  page,
}) => {
  await page.setContent(`
    <div data-testid="bad" style="padding:7px"><span>x</span></div>
    <div data-testid="good" style="padding:8px"><span>x</span></div>
  `)
  // Guards the dead-tolerance regression: 7px must be flagged.
  expect(has(await violationsFor(page, 'bad', 'spacingScale'), 'spacingScale')).toBe(true)
  expect(has(await violationsFor(page, 'good', 'spacingScale'), 'spacingScale')).toBe(false)
})

test('spacingScale fires on off-ramp radius, not on pill/on-ramp', async ({
  page,
}) => {
  await page.setContent(`
    <div data-testid="bad" style="border-radius:7px">x</div>
    <div data-testid="ramp" style="border-radius:8px">x</div>
    <div data-testid="pill" style="border-radius:9999px">x</div>
  `)
  expect(has(await violationsFor(page, 'bad', 'spacingScale'), 'spacingScale')).toBe(true)
  expect(has(await violationsFor(page, 'ramp', 'spacingScale'), 'spacingScale')).toBe(false)
  expect(has(await violationsFor(page, 'pill', 'spacingScale'), 'spacingScale')).toBe(false)
})

test('childOverflow fires horizontally AND vertically; not when parent scrolls', async ({
  page,
}) => {
  await page.setContent(`
    <div data-testid="hbad" style="width:100px;overflow:visible">
      <div style="width:200px;height:10px">wide</div>
    </div>
    <div data-testid="vbad" style="height:20px;width:100px;overflow:visible">
      <div style="height:80px;width:10px">tall</div>
    </div>
    <div data-testid="scroll" style="width:100px;overflow:auto">
      <div style="width:200px;height:10px">wide</div>
    </div>
  `)
  expect(has(await violationsFor(page, 'hbad', 'childOverflow'), 'childOverflow')).toBe(true)
  expect(has(await violationsFor(page, 'vbad', 'childOverflow'), 'childOverflow')).toBe(true)
  expect(has(await violationsFor(page, 'scroll', 'childOverflow'), 'childOverflow')).toBe(false)
})

test('textTruncation fires on clip-without-ellipsis; not with line-clamp', async ({
  page,
}) => {
  await page.setContent(`
    <div data-testid="bad" style="width:40px;overflow:hidden;white-space:nowrap">
      averylongunbrokenstringthatclips
    </div>
    <div data-testid="clamp" style="width:40px;overflow:hidden;display:-webkit-box;-webkit-line-clamp:1;-webkit-box-orient:vertical">
      averylongunbrokenstringthatclips
    </div>
  `)
  expect(has(await violationsFor(page, 'bad', 'textTruncation'), 'textTruncation')).toBe(true)
  expect(has(await violationsFor(page, 'clamp', 'textTruncation'), 'textTruncation')).toBe(false)
})

test('buttonWidth fires on a non-block button spanning a wide container among siblings', async ({
  page,
}) => {
  await page.setContent(`
    <div data-testid="bad" style="width:400px">
      <button style="width:400px;display:inline-flex">spans</button>
      <span>sibling</span>
    </div>
    <div data-testid="ok" style="width:400px">
      <button style="display:inline-flex">normal</button>
      <span>sibling</span>
    </div>
  `)
  expect(has(await violationsFor(page, 'bad', 'buttonWidth'), 'buttonWidth')).toBe(true)
  expect(has(await violationsFor(page, 'ok', 'buttonWidth'), 'buttonWidth')).toBe(false)
})

test('horizontalScroll fires when the document overflows its width', async ({
  page,
}) => {
  await page.setViewportSize({ width: 400, height: 400 })
  await page.setContent(
    `<div data-testid="root"><div style="width:900px;height:10px">wide</div></div>`,
  )
  const v = await assertLayoutSane(page.getByTestId('root'), {
    checks: {
      childOverflow: false,
      siblingOverlap: false,
      spacingScale: false,
      buttonWidth: false,
      touchTarget: false,
      textTruncation: false,
    },
    collect: true,
  })
  expect(has(v, 'horizontalScroll')).toBe(true)
})

test('a clean DOM produces no violations (no false positives)', async ({
  page,
}) => {
  await page.setContent(`
    <div data-testid="clean" style="padding:8px;display:flex;gap:8px;border-radius:8px">
      <button style="display:inline-flex;padding:8px 16px;border-radius:8px">OK</button>
      <span style="padding:4px">label</span>
    </div>
  `)
  const v = await assertLayoutSane(page.getByTestId('clean'), {
    checks: { horizontalScroll: false },
    collect: true,
  })
  expect(v, JSON.stringify(v)).toHaveLength(0)
})
