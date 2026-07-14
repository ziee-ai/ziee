import { test } from 'node:test'
import assert from 'node:assert/strict'
import { GALLERY_COVERAGE } from '@/dev/gallery/coverage.ts'

// TEST-25 / TEST-44 (split-chat ITEM-15/23): the split surfaces cannot be seeded
// as standalone gallery stories — the backend-free gallery can't drive two live,
// independently-streaming panes (DRIFT-1.12). Instead they declare `kind: 'via'`
// coverage entries so `check:gallery-coverage` is satisfied WITHOUT a dedicated
// multi-pane cell, and the runtime behavior is proven by the 14-split-chat e2e
// specs. This test locks that contract in: the split container + the pane
// context provider + the pop-out action each carry a documented `via` entry.

const cov = GALLERY_COVERAGE as Record<
  string,
  { kind: string; reason?: string } | undefined
>

for (const key of [
  'modules/chat/components/SplitChatView',
  'modules/chat/core/pane/ChatPaneContext',
  'modules/chat/components/OpenInNewWindowAction',
]) {
  test(`gallery-coverage: ${key} is covered 'via' (no standalone multi-pane cell needed)`, () => {
    const entry = cov[key]
    assert.ok(entry, `${key} must have a GALLERY_COVERAGE entry`)
    assert.equal(
      entry.kind,
      'via',
      `${key} is exercised via the chat surface + e2e, not a standalone story`,
    )
    assert.ok(
      typeof entry.reason === 'string' && entry.reason.length > 0,
      `${key} 'via' entry must document WHY (the gallery-coverage contract)`,
    )
  })
}
