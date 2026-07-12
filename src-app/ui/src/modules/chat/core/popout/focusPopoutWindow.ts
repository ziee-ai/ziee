/**
 * Web base of `focusPopoutWindowIfOpen` (ITEM-53 / FB-12).
 *
 * On the web there are no native pop-out WINDOWS (the web pop-out is a browser tab
 * the OS/browser already manages), so there is nothing for the main app to focus —
 * this is a no-op that returns `false`, meaning "I did not handle it; open the
 * conversation inline as usual". The desktop override
 * (`focusPopoutWindow.desktop.ts`) checks for a live `WebviewWindow` and focuses it.
 *
 * @returns `true` if an existing pop-out window was focused (so the caller must NOT
 *   open the conversation inline); `false` otherwise.
 */
export async function focusPopoutWindowIfOpen(
  _conversationId: string,
): Promise<boolean> {
  return false
}
