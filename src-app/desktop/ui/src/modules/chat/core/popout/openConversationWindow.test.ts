import { describe, it, expect, beforeEach, vi } from 'vitest'

// TEST-P5 (split-chat pop-out, desktop override, ITEM-P1/P4): on the Tauri
// desktop, opening a conversation in a new window spawns a NATIVE `WebviewWindow`
// labelled `chat-<id>` at `/chat/<id>`; reopening the same id focuses the
// existing labelled window instead of duplicating (the same dedup contract as the
// web window-name). Re-scoped from a Tauri-GUI e2e (which needs a display) to a
// unit of the desktop override — the window-API contract is exactly what this
// file owns. The Tauri module is mocked so the override's branching is exercised
// without a running Tauri runtime.

const h = vi.hoisted(() => ({
  ctorCalls: [] as Array<{ label: string; options: Record<string, unknown> }>,
  state: { existing: null as null | { setFocus: () => void; unminimize: () => void } },
}))

vi.mock('@tauri-apps/api/webviewWindow', () => {
  class WebviewWindow {
    label: string
    options: Record<string, unknown>
    once = () => {}
    constructor(label: string, options: Record<string, unknown>) {
      this.label = label
      this.options = options
      h.ctorCalls.push({ label, options })
    }
    static getByLabel = async (_label: string) => h.state.existing
  }
  return { WebviewWindow }
})

import { openConversationWindow } from './openConversationWindow.ts'

beforeEach(() => {
  h.ctorCalls.length = 0
  h.state.existing = null
})

describe('desktop openConversationWindow (TEST-P5)', () => {
  it('opens a native WebviewWindow labelled chat-<id> at /chat/<id> when none exists', async () => {
    await openConversationWindow('c1', { title: 'My Chat' })
    expect(h.ctorCalls).toHaveLength(1)
    expect(h.ctorCalls[0].label).toBe('chat-c1')
    expect(h.ctorCalls[0].options.url).toBe('/chat/c1')
    expect(h.ctorCalls[0].options.title).toBe('My Chat')
    expect(h.ctorCalls[0].options.resizable).toBe(true)
  })

  it('focuses + unminimizes the existing window and does NOT duplicate (dedup by label)', async () => {
    const setFocus = vi.fn()
    const unminimize = vi.fn()
    h.state.existing = { setFocus, unminimize }
    await openConversationWindow('c1')
    expect(setFocus).toHaveBeenCalledTimes(1)
    expect(unminimize).toHaveBeenCalledTimes(1)
    expect(h.ctorCalls).toHaveLength(0)
  })

  it('a DIFFERENT conversation id gets a DIFFERENT label (not deduped onto the first)', async () => {
    await openConversationWindow('c1')
    await openConversationWindow('c2')
    expect(h.ctorCalls.map((c) => c.label)).toEqual(['chat-c1', 'chat-c2'])
  })
})
