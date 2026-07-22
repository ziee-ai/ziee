/**
 * Composer model selector — a long model name must READ, at every composer width.
 *
 * The bug: the trigger was pinned at `max-w-[130px]` and had no working ellipsis,
 * so a name like "GPT OSS 120B" was HARD-CUT into the chevron. The missing
 * ellipsis was not an oversight in the app — the kit's shadcn trigger sets BOTH
 * `*:data-[slot=select-value]:line-clamp-1` and `*:data-[slot=select-value]:flex`
 * on the value slot, and Tailwind orders `display` after `line-clamp`, so
 * `display:flex` wins, `-webkit-line-clamp`'s ellipsis goes inert, and only its
 * `overflow:hidden` survives. `text-overflow: ellipsis` does not apply to a flex
 * container, so the fix renders the selected label as a block-level truncating
 * span through the kit's public `labelRender` seam.
 *
 * These specs assert the EFFECT in BOTH regimes, because the fix is not "a bigger
 * cap" — it is that the trigger sizes to its CONTENT:
 *   - wide composer   → the name renders IN FULL, with no ellipsis at all;
 *   - narrow composer → it ellipsizes, and SEND is not what gave way.
 * Asserting only one regime would pass for a fix that regressed the other (a
 * permanently-ellipsized trigger, or one that shoves Send off the edge).
 *
 * Class strings are deliberately NOT asserted: any equivalent re-implementation
 * should keep these green, and asserting `max-w-[20rem]` would not have caught
 * the original bug (the old class string looked perfectly reasonable too).
 */
import { expect, test, type Page } from '@playwright/test'
import { STANDALONE_PATH, THEMES } from './_gallery'

/**
 * TWO surfaces, because the fix has two regimes and one name cannot show both.
 *
 * `FITS` seeds a realistic long name that stays inside the trigger's soft
 * ceiling; `OVERLONG` seeds one past it. Measured while writing this spec: the
 * FITS name occupies ~217px and does NOT truncate even at a 390px viewport —
 * the composer's left group is `flex-1` with a 0 flex-basis, so it absorbs the
 * pressure long before the right group is squeezed. That is the intended
 * outcome of content sizing (after this fix, truncation is genuinely rare), but
 * it does mean the ellipsis path is only reachable via a name that exceeds the
 * ceiling — which is exactly when it should engage.
 */
const SURFACE_FITS = 'deep-chat-long-model-name'
const SURFACE_OVERLONG = 'deep-chat-overlong-model-name'

/**
 * The model names seeded by those surfaces. Duplicated from
 * `src/modules/chat/gallery.tsx` rather than imported — the visual specs stay
 * decoupled from the app module graph (see `_gallery.ts`). If a seed changes,
 * these must change with it; the `toBe(LONG_MODEL_NAME)` /
 * `toContain(OVERLONG_MODEL_NAME)` assertions fail loudly if they drift apart.
 */
const LONG_MODEL_NAME = 'GPT OSS 120B Instruct Turbo'
const OVERLONG_MODEL_NAME = 'DeepSeek R1 Distill Llama 70B Instruct Turbo Preview'
const OVERLONG_MODEL_ID = 'gallery-overlong-model'

/** The width the bug was pinned at. The trigger must now exceed it when there is room. */
const OLD_FIXED_CAP_PX = 130
/** The `max-w-[20rem]` soft ceiling, in px, that bounds a pathological name. */
const SOFT_CEILING_PX = 320

/** Both taken from the gallery matrix's own VIEWPORTS (`_gallery.ts`). */
const WIDE = { width: 1280, height: 900 }
const NARROW = { width: 390, height: 844 }

interface Measurement {
  /** Trigger button border-box width. */
  triggerW: number
  /** The truncating label span inside the trigger's value slot. */
  label: {
    text: string
    scrollW: number
    clientW: number
    textOverflow: string
    display: string
    right: number
  }
  /** The trigger's CONTENT-box right edge — the label must not cross it. */
  triggerContentRight: number
  send: { width: number; right: number }
  composerRight: number
  /**
   * The composer's "+" button — the left toolbar group's `shrink-0` content.
   * The left group is `flex-1` with a ZERO flex-basis, so it only receives space
   * the right group leaves over; an unbounded content-sized model selector
   * starves it to ~0 and this button overflows INTO the selector. Measuring its
   * right edge against the trigger's left edge is what catches that.
   */
  plusRight: number
  triggerLeft: number
  /** Composer width — the space the toolbar row actually has to divide up. */
  composerW: number
}

async function openSurface(
  page: Page,
  opts: { surface: string; name: string; theme: string; vp: typeof WIDE },
): Promise<void> {
  await page.setViewportSize(opts.vp)
  const q = new URLSearchParams({ surface: opts.surface, theme: opts.theme, accent: 'blue' })
  await page.goto(`${STANDALONE_PATH}?${q.toString()}`)
  await page.getByTestId('ullm-model-select').waitFor({ state: 'visible' })
  await page.waitForFunction(t => document.documentElement.classList.contains(t), opts.theme)
  await page.evaluate(() => document.fonts?.ready)
  // The gallery seed re-asserts the held ModelPicker state on an interval, so
  // wait for the SELECTED name to actually be on the trigger rather than
  // measuring a pre-seed frame (which would report a placeholder width).
  // `textContent` is the untruncated DOM text, so this works in both regimes.
  await page.waitForFunction(
    name =>
      document
        .querySelector('[data-testid="ullm-model-select"]')
        ?.textContent?.includes(name) ?? false,
    opts.name,
    { timeout: 15_000 },
  )
}

const openFits = (page: Page, theme: string, vp: typeof WIDE) =>
  openSurface(page, { surface: SURFACE_FITS, name: LONG_MODEL_NAME, theme, vp })

const openOverlong = (page: Page, theme: string, vp: typeof WIDE) =>
  openSurface(page, { surface: SURFACE_OVERLONG, name: OVERLONG_MODEL_NAME, theme, vp })

async function measure(page: Page): Promise<Measurement> {
  return page.evaluate(() => {
    const trigger = document.querySelector<HTMLElement>(
      '[data-testid="ullm-model-select"]',
    )
    if (!trigger) throw new Error('no model-select trigger on this surface')
    const value = trigger.querySelector<HTMLElement>('[data-slot="select-value"]')
    if (!value) throw new Error('the trigger has no select-value slot')
    // The truncating span the fix introduces. Fall back to the value slot itself
    // so a REMOVED span reports the (untruncated) slot and fails the assertions
    // loudly, instead of throwing something unrelated to the defect.
    const label = (value.querySelector<HTMLElement>('span') ?? value) as HTMLElement
    const send = document.querySelector<HTMLElement>(
      '[data-testid="chat-input-send-btn"]',
    )
    if (!send) throw new Error('no send button on this surface')
    const plus = document.querySelector<HTMLElement>(
      '[data-testid="chat-input-add-btn"]',
    )
    if (!plus) throw new Error('no "+" toolbar button on this surface')
    const composer = send.closest<HTMLElement>('[data-chat-composer]')
    if (!composer) throw new Error('the send button is not inside a composer')

    const ts = getComputedStyle(trigger)
    const tr = trigger.getBoundingClientRect()
    const ls = getComputedStyle(label)
    const px = (v: string) => parseFloat(v) || 0
    return {
      triggerW: +tr.width.toFixed(2),
      label: {
        text: (label.textContent ?? '').trim(),
        scrollW: label.scrollWidth,
        clientW: label.clientWidth,
        textOverflow: ls.textOverflow,
        display: ls.display,
        right: +label.getBoundingClientRect().right.toFixed(2),
      },
      // Border box minus the trigger's own border + padding on the end side.
      triggerContentRight: +(
        tr.right -
        px(ts.borderRightWidth) -
        px(ts.paddingRight)
      ).toFixed(2),
      send: {
        width: +send.getBoundingClientRect().width.toFixed(2),
        right: +send.getBoundingClientRect().right.toFixed(2),
      },
      composerRight: +composer.getBoundingClientRect().right.toFixed(2),
      plusRight: +plus.getBoundingClientRect().right.toFixed(2),
      triggerLeft: +tr.left.toFixed(2),
      composerW: +composer.getBoundingClientRect().width.toFixed(2),
    }
  })
}

test.describe('composer model selector — long name', () => {
  for (const theme of THEMES) {
    test(`TEST-2: a long model name renders IN FULL at a wide composer (${theme})`, async ({
      page,
    }) => {
      await openFits(page, theme, WIDE)
      const m = await measure(page)

      // The whole point of the fix: with room available there is NO truncation.
      // A regression to any fixed cap (including a merely-larger one) fails here.
      expect(
        m.label.text,
        'the trigger must show the seeded model name (gallery seed drift?)',
      ).toBe(LONG_MODEL_NAME)
      expect(
        m.label.scrollW,
        `the label is truncated at a WIDE composer (${m.label.scrollW}px of text ` +
          `in ${m.label.clientW}px) — the trigger is not sizing to its content`,
      ).toBeLessThanOrEqual(m.label.clientW + 1)

      // It is content-sized, not re-pinned: wider than the old 130px cap, and
      // still inside the soft ceiling that stops a pathological name.
      expect(
        m.triggerW,
        'the trigger is still at/below the old fixed cap — content sizing is not in effect',
      ).toBeGreaterThan(OLD_FIXED_CAP_PX)
      expect(
        m.triggerW,
        'the trigger exceeded its soft ceiling — a long name can swallow the toolbar',
      ).toBeLessThanOrEqual(SOFT_CEILING_PX)

      // Even when nothing is clipped, Send must be fully inside the composer.
      expect(m.send.right).toBeLessThanOrEqual(m.composerRight + 1)
    })
  }

  test('TEST-3: a name past the soft ceiling ellipsizes instead of being hard-cut', async ({
    page,
  }) => {
    await openOverlong(page, 'light', WIDE)
    const m = await measure(page)

    // The ceiling bound the trigger rather than letting the name run away with
    // the toolbar — this is what makes the generous `w-auto` safe.
    expect(
      m.triggerW,
      'an over-long name pushed the trigger past its soft ceiling',
    ).toBeLessThanOrEqual(SOFT_CEILING_PX)

    // Guard: if it happens to FIT the assertions below would pass vacuously and
    // prove nothing about truncation.
    expect(
      m.label.scrollW,
      'the seeded "over-long" name fits inside the ceiling, so this spec cannot ' +
        'prove truncation — lengthen the gallery seed',
    ).toBeGreaterThan(m.label.clientW)

    // BOTH conditions are required. The pre-fix state clipped too
    // (`overflow:hidden` survived) but drew NO ellipsis, because the value slot
    // is a flex container and `text-overflow` does not apply to one — so the
    // ellipsis property is the half that was actually missing.
    expect(
      m.label.textOverflow,
      'the label has no text-overflow: ellipsis — a long name is hard-cut, not ellipsized',
    ).toBe('ellipsis')
    expect(
      m.label.display,
      'the label must be a BLOCK container for text-overflow to apply (a flex ' +
        'item renders the ellipsis inert — this was the original defect)',
    ).toBe('block')

    // And it stays inside the trigger rather than spilling over the chevron.
    expect(
      m.label.right,
      `the label overruns the trigger's content box by ` +
        `${(m.label.right - m.triggerContentRight).toFixed(1)}px — it overlays the chevron`,
    ).toBeLessThanOrEqual(m.triggerContentRight + 1)
  })

  test('TEST-4: under pressure the model name yields, never the Send button', async ({
    page,
  }) => {
    // The over-long surface: the only one where the right group genuinely comes
    // under pressure, so "who yields" is a real question rather than moot.
    await openOverlong(page, 'light', WIDE)
    const wide = await measure(page)
    await openOverlong(page, 'light', NARROW)
    const narrow = await measure(page)

    // Send is `shrink-0`: identical width in both regimes, and always fully
    // inside the composer. This is the assertion that would fail if the
    // shrink-0 were left on the whole right GROUP (the group would refuse to
    // shrink and push Send past the edge instead).
    expect(
      narrow.send.width,
      `Send shrank from ${wide.send.width}px to ${narrow.send.width}px — it must ` +
        `never be what yields`,
    ).toBeCloseTo(wide.send.width, 1)
    expect(
      narrow.send.right,
      'Send is clipped by / pushed outside the composer at a narrow width',
    ).toBeLessThanOrEqual(narrow.composerRight + 1)

    // ...and the model trigger IS what gave way, so the comparison is meaningful
    // rather than a pair of unrelated measurements.
    expect(
      narrow.triggerW,
      `the model trigger did not shrink at a narrow composer — something else ` +
        `absorbed the pressure (composer ${wide.composerW}px → ${narrow.composerW}px)`,
    ).toBeLessThan(wide.triggerW)

    // The other thing that must not be what yields: the LEFT toolbar group.
    // It is `flex-1` with a zero basis, so it receives only what the right group
    // leaves over — an unbounded content-sized selector starves it to ~0 and its
    // `shrink-0` "+" button overflows INTO the selector. (Observed exactly that
    // at 390px before the left group was given a min-width floor.)
    for (const [label, m] of [
      ['wide', wide],
      ['narrow', narrow],
    ] as const) {
      expect(
        m.plusRight,
        `[${label}] the "+" button overlaps the model selector by ` +
          `${(m.plusRight - m.triggerLeft).toFixed(1)}px — the model name grew ` +
          `until it starved the left toolbar group`,
      ).toBeLessThanOrEqual(m.triggerLeft + 1)
    }

    // ...and it must not yield TOO MUCH either. Protecting the left group with a
    // `max-w` CEILING on the right group instead of a min-width FLOOR on the left
    // one reserves space unconditionally: the left group is `flex-1` with a zero
    // basis, so it grows into whatever the ceiling leaves spare even when it holds
    // nothing but the 36px "+" button. Measured at 390px under a 60% ceiling, that
    // stranded ~90px of empty gutter between "+" and the model name while the name
    // itself was ellipsized — truncating far earlier than the row required. So
    // when the name IS under pressure, the space between them must stay of the
    // order of the toolbar's own gaps.
    const gutter = narrow.triggerLeft - narrow.plusRight
    expect(
      gutter,
      `${gutter.toFixed(1)}px of empty space sits between the "+" button and an ` +
        `ELLIPSIZED model name — the name is yielding space that nothing is using`,
    ).toBeLessThanOrEqual(24)
  })

  test('TEST-5: the OPEN list still shows the full name while the trigger is truncated', async ({
    page,
  }) => {
    await openOverlong(page, 'light', WIDE)
    const before = await measure(page)
    // Precondition: we are in the truncated regime, so this proves the name is
    // still RECOVERABLE precisely when the trigger cannot show it.
    expect(before.label.scrollW).toBeGreaterThan(before.label.clientW)

    await page.getByTestId('ullm-model-select').click()
    const option = page.getByTestId(`ullm-model-select-opt-${OVERLONG_MODEL_ID}`)
    await option.waitFor({ state: 'visible' })

    const opt = await option.evaluate(el => ({
      text: (el.textContent ?? '').trim(),
      scrollW: el.scrollWidth,
      clientW: el.clientWidth,
    }))
    expect(opt.text).toContain(OVERLONG_MODEL_NAME)
    expect(
      opt.scrollW,
      'the dropdown row is ALSO truncating the name — with the trigger ' +
        'ellipsized there is then no way to read the selected model',
    ).toBeLessThanOrEqual(opt.clientW + 1)
  })
})
