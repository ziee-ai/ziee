/**
 * Open a conversation in a NEW top-level window/tab, fully independent of the
 * current one.
 *
 * Web (this file): a new browser window/tab. It boots a fresh copy of the SPA
 * with its OWN singleton stores, its own SSE stream, and its own extensions — so
 * it is independent by construction, with no chat-store refactor needed. The
 * window NAME `chat-<id>` gives dedup for free: `window.open` with an existing
 * target name focuses/reuses that window instead of opening a duplicate.
 *
 * Desktop OVERRIDES this file via the co-located `./openConversationWindow.desktop.ts`
 * (the live2 `.desktop` override mechanism — resolved by `localOverridePlugin` in the
 * desktop bundle) to open a native Tauri `WebviewWindow` instead. Opening a native OS
 * window is shell-native, so the desktop copy uses the Tauri window API directly (NOT
 * an Axum route).
 */
export async function openConversationWindow(
  conversationId: string,
  _opts?: { title?: string },
): Promise<void> {
  const name = `chat-${conversationId}`
  const win = window.open(`/chat/${conversationId}`, name)
  // If the browser reused an existing tab (same name) it may already be focused;
  // best-effort focus for the fresh-open case. A popup blocker returns null.
  win?.focus()
}
