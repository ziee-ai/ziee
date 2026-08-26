/**
 * TEST-6 — the dev-gallery cassette seed for the background module (ITEM-4).
 *
 * Guards that the populated Transcript state the card now exposes is actually
 * SEEDED, so the `check:state-matrix` + `gate:ui` runtime render it with real
 * data (a completed sub-agent run WITH a transcript), and that the designated
 * empty-tasks conversation still resolves to zero runs. A regression that drops
 * the `activity` seed would silently leave the Transcript tab empty in every
 * gallery cell — this fails first.
 *
 *   npx vitest run src/modules/background/gallery.test.tsx
 */
import { describe, expect, test } from 'vitest'
import { gallery, GALLERY_EMPTY_TASKS_CONVERSATION_ID } from './gallery'

// The completed sub-agent fixture id (mirrors `SUBAGENT_DONE` in gallery.tsx).
const SUBAGENT_DONE = 'b0000000-0000-0000-0000-000000000002'

// Minimal cassette ctx — only the fields these two resolvers read. The cassette
// map is a UNION of per-endpoint resolver signatures, so each entry is cast to a
// plain callable for the test.
type Ctx = { params: Record<string, string>; query: Record<string, string> }
const ctx = (over: Partial<Ctx>): Ctx => ({ params: {}, query: {}, ...over })
type Resolver = (c: Ctx) => unknown
const resolver = (key: string): Resolver => {
  const fn = gallery.cassette?.[key as keyof NonNullable<typeof gallery.cassette>]
  expect(fn).toBeTruthy()
  return fn as unknown as Resolver
}

describe('background gallery cassette (ITEM-4)', () => {
  test('the completed sub-agent detail is seeded WITH a non-empty transcript', () => {
    const detail = resolver('Background.getRun')(
      ctx({ params: { run_id: SUBAGENT_DONE } }),
    ) as { activity?: unknown[] }
    expect(Array.isArray(detail.activity)).toBe(true)
    expect((detail.activity ?? []).length).toBeGreaterThan(0)
  })

  test('the designated empty-tasks conversation resolves to zero runs', () => {
    const listRuns = resolver('Background.listRuns')
    const empty = listRuns(
      ctx({ query: { conversation_id: GALLERY_EMPTY_TASKS_CONVERSATION_ID } }),
    ) as { total: number; runs: unknown[] }
    expect(empty.total).toBe(0)
    expect(empty.runs.length).toBe(0)

    const populated = listRuns(
      ctx({ query: { conversation_id: 'c0000000-0000-0000-0000-000000000009' } }),
    ) as { runs: unknown[] }
    expect(populated.runs.length).toBeGreaterThan(0)
  })
})
