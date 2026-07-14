import { describe, it, expect, beforeEach, vi } from 'vitest'

// TEST-83 (ITEM-54 / FB-12): the desktop cross-window snap-back WIRING control flow,
// with the Tauri boundary (window + event bus) mocked so the real branching runs
// without a Tauri runtime. Proves: a pop-out window EMITS the close signal with its
// conversationId on close, and the main window's LISTENER runs the snap-back handler
// (opening the conversation back as a pane) when that signal arrives. The actual
// cross-OS-window event DELIVERY is a Tauri platform guarantee (desktop-host
// verified), not logic this test owns.

const h = vi.hoisted(() => ({
  emitted: [] as Array<{ event: string; payload: unknown }>,
  closeHandler: null as null | (() => Promise<void> | void),
  listeners: {} as Record<string, (e: { payload: unknown }) => void>,
}))

vi.mock('@tauri-apps/api/webviewWindow', () => ({
  getCurrentWebviewWindow: () => ({
    onCloseRequested: async (fn: () => Promise<void> | void) => {
      h.closeHandler = fn
      return () => {}
    },
  }),
}))
vi.mock('@tauri-apps/api/event', () => ({
  emit: async (event: string, payload: unknown) => {
    h.emitted.push({ event, payload })
  },
  listen: async (event: string, cb: (e: { payload: unknown }) => void) => {
    h.listeners[event] = cb
    return () => {}
  },
}))

import {
  registerPopoutCloseEmitter,
  registerMainWindowSnapBackListener,
  POPOUT_CLOSED_EVENT,
} from '@/modules/chat/core/popout/popoutSnapBack.desktop'

beforeEach(() => {
  h.emitted = []
  h.closeHandler = null
  h.listeners = {}
})

describe('desktop popoutSnapBack wiring (TEST-83)', () => {
  it('a pop-out window emits POPOUT_CLOSED with its conversationId when it closes', async () => {
    await registerPopoutCloseEmitter('conv-9')
    expect(typeof h.closeHandler).toBe('function')
    await h.closeHandler!() // simulate the window closing
    expect(h.emitted).toEqual([
      { event: POPOUT_CLOSED_EVENT, payload: { conversationId: 'conv-9' } },
    ])
  })

  it('the main-window listener runs the snap-back handler on a POPOUT_CLOSED event', async () => {
    const opened: string[] = []
    await registerMainWindowSnapBackListener({
      getPaneConversationIds: () => ['a', 'b'],
      getSinglePaneConversationId: () => null,
      maxPanes: 3,
      openAsNewPane: id => opened.push(id),
    })
    const cb = h.listeners[POPOUT_CLOSED_EVENT]
    expect(typeof cb).toBe('function')
    cb({ payload: { conversationId: 'conv-9' } })
    expect(opened).toEqual(['conv-9']) // snapped back as a new pane
  })
})
