import { describe, it, expect, beforeEach, vi } from 'vitest'

// TEST-80 (ITEM-53 / FB-12): dedup-focus from the MAIN window. When a conversation
// is already live in a native pop-out window, opening it from the main window must
// FOCUS that existing window (unminimize + setFocus) and report handled=true so the
// caller aborts the inline open; when no such window exists it returns false (open
// inline as usual). The Tauri module is mocked so the seam's real branching runs
// without a Tauri runtime — the desktop-window contract this seam owns.

const h = vi.hoisted(() => ({
  getByLabelArg: null as string | null,
  state: {
    existing: null as null | { setFocus: () => void; unminimize: () => void },
  },
}))

vi.mock('@tauri-apps/api/webviewWindow', () => {
  class WebviewWindow {
    static getByLabel = async (label: string) => {
      h.getByLabelArg = label
      return h.state.existing
    }
  }
  return { WebviewWindow }
})

import { focusPopoutWindowIfOpen } from '@/modules/chat/core/popout/focusPopoutWindow.desktop'
import { popoutWindowLabel } from '@/modules/chat/core/popout/popoutWindowLabel'

beforeEach(() => {
  h.getByLabelArg = null
  h.state.existing = null
})

describe('desktop focusPopoutWindowIfOpen (TEST-80)', () => {
  it('an already-open pop-out window is unminimized + focused; returns true (abort inline open)', async () => {
    const setFocus = vi.fn()
    const unminimize = vi.fn()
    h.state.existing = { setFocus, unminimize }

    const handled = await focusPopoutWindowIfOpen('c1')

    expect(handled).toBe(true)
    expect(h.getByLabelArg).toBe('chat-c1') // looked up by the SHARED label
    expect(unminimize).toHaveBeenCalledTimes(1)
    expect(setFocus).toHaveBeenCalledTimes(1)
    // Unminimize MUST precede focus (some WMs ignore setFocus on a minimized window).
    expect(unminimize.mock.invocationCallOrder[0]).toBeLessThan(
      setFocus.mock.invocationCallOrder[0],
    )
  })

  it('no existing window for the conversation → returns false (open inline as usual)', async () => {
    h.state.existing = null
    expect(await focusPopoutWindowIfOpen('c2')).toBe(false)
    expect(h.getByLabelArg).toBe('chat-c2')
  })

  it('keys off the SAME label as openConversationWindow (shared popoutWindowLabel)', () => {
    expect(popoutWindowLabel('abc')).toBe('chat-abc')
  })
})
