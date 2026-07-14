import { test, beforeEach, afterEach } from 'node:test'
import assert from 'node:assert/strict'
import { openConversationWindow } from './openConversationWindow.ts'

// TEST-P1 / TEST-P2 (split-chat pop-out, ITEM-P1/P4): the WEB pop-out opens the
// conversation in a new browser window/tab named `chat-<id>`, so the URL is
// right and the per-conversation window NAME gives dedup for free (window.open
// with an existing target name reuses/focuses that window). node:test has no DOM,
// so we stub the minimal `window.open` surface and record the calls.

let calls: Array<{ url: string; name: string; focused: boolean }> = []
let origWindow: unknown

beforeEach(() => {
  calls = []
  origWindow = (globalThis as { window?: unknown }).window
  ;(globalThis as { window?: unknown }).window = {
    open(url: string, name: string) {
      const handle = {
        url,
        name,
        focused: false,
        focus(this: { focused: boolean }) {
          this.focused = true
        },
      }
      calls.push(handle)
      return handle
    },
  }
})

afterEach(() => {
  ;(globalThis as { window?: unknown }).window = origWindow
})

test('TEST-P1: opens /chat/<id> with a per-conversation window name and focuses it', async () => {
  await openConversationWindow('conv-1')
  assert.equal(calls.length, 1, 'window.open called once')
  assert.equal(calls[0].url, '/chat/conv-1', 'navigates to the conversation route')
  assert.equal(calls[0].name, 'chat-conv-1', 'window name is chat-<id>')
  assert.equal(calls[0].focused, true, 'the fresh window is focused')
})

test('TEST-P2: a second open for the same id reuses the same window name (dedup, no duplicate)', async () => {
  await openConversationWindow('conv-1')
  await openConversationWindow('conv-1')
  assert.equal(calls[0].name, 'chat-conv-1')
  assert.equal(
    calls[0].name,
    calls[1].name,
    'same target name → the browser focuses/reuses the existing window instead of duplicating',
  )
  // A DIFFERENT conversation gets a DIFFERENT name (so it is NOT deduped onto the first).
  await openConversationWindow('conv-2')
  assert.equal(calls[2].name, 'chat-conv-2')
  assert.notEqual(calls[2].name, calls[0].name)
})
