/**
 * Chat Extensions Auto-Discovery
 *
 * Three supported registration paths (any combination, same registry):
 *
 *   1. In-chat extensions (this folder):
 *        `chat/extensions/<name>/extension.tsx`
 *      Picked up by the first glob below. Use for extensions that
 *      conceptually belong TO chat (text rendering, title generation,
 *      keyboard shortcuts, export, model picker, syntax highlight,
 *      file). Memory, assistant, and mcp have been promoted to
 *      sibling-module bridges (see Path 2).
 *
 *   2. Sibling-module extensions (anywhere under modules/):
 *        `modules/<module-name>/chat-extension/extension.tsx`
 *      Picked up by the second glob below. Use when another module
 *      owns a bridge into chat (e.g. projects → chat). Keeps chat's
 *      folder free of project-aware code while still requiring zero
 *      wiring on the module's side — just drop the file at the
 *      conventional path.
 *
 *   3. Manual registration (anywhere):
 *        `chatExtensionRegistry.register(myExtension)`
 *      Call from your module's init code. Use when you need
 *      programmatic control (feature flag, conditional registration)
 *      or your file layout doesn't match either glob convention.
 *
 * Each `extension.tsx` exports a default `ChatExtension`. Re-registration
 * is HMR-safe — the registry warns and unregisters first, so paths can
 * overlap without crashing.
 *
 * Example extension:
 * ```typescript
 * import { createExtension, type ChatExtension } from '@/modules/chat/core/extensions'
 *
 * export default createExtension({
 *   name: 'myextension',
 *   description: 'My custom extension',
 *   priority: 50,
 * })
 * ```
 *
 * Bootstrapped by `chat/module.tsx` via `import '@/modules/chat/extensions'`.
 */

import { useEffect, useState } from 'react'
import { chatExtensionRegistry } from '@/modules/chat/core/extensions'
import type { ChatExtension } from '@/modules/chat/core/extensions'

// LAZY globs (no `{ eager: true }`): each extension.tsx becomes its OWN chunk
// instead of being inlined into the eager chat module → the entry chunk. Every
// extension.tsx statically imports its renderer components, which import their
// stores, so an EAGER glob dragged that whole subtree (components + stores) into
// entry. Lazy-globbing keeps it out. Registration still AUTO-STARTS at boot (the
// IIFE below runs when chat/module.tsx imports this), so extensions are
// registered well before the lazy `/chat` route renders; `chatExtensionsReady`
// is exported for anything that must await it.
const inChatExtensions = import.meta.glob<{ default: ChatExtension }>(
  './*/extension.tsx',
)
const siblingModuleExtensions = import.meta.glob<{ default: ChatExtension }>(
  '../../*/chat-extension/extension.tsx',
)

/** Resolves once every discovered chat-extension has been registered. */
export const chatExtensionsReady: Promise<void> = (async () => {
  const loaders = { ...inChatExtensions, ...siblingModuleExtensions }
  const modules = await Promise.all(
    Object.entries(loaders).map(async ([path, load]) => ({
      path,
      ext: (await load()).default,
    })),
  )
  const discoveredExtensions: ChatExtension[] = []
  for (const { path, ext } of modules) {
    if (!ext) continue
    discoveredExtensions.push(ext)
    console.log(`[Chat Extensions] Discovered: ${ext.name} (${path})`)
  }
  discoveredExtensions.sort((a, b) => (a.priority ?? 100) - (b.priority ?? 100))
  for (const extension of discoveredExtensions) {
    chatExtensionRegistry.register(extension, { enabled: true })
  }
  console.log(
    '[Chat Extensions] Registered',
    chatExtensionRegistry.getExtensions().length,
    'extensions',
  )
})()

/**
 * Re-export extension registry for convenience
 */
export { chatExtensionRegistry } from '../core/extensions'

/**
 * Gate a chat surface on chat-extension registration.
 *
 * Importing this module kicks off discovery (the IIFE above), so the FIRST chat
 * page that calls this hook triggers the lazy load of all chat extensions + their
 * stores. Returns `false` until `chatExtensionsReady` resolves — chat composers
 * gate on it so the request-field composers (file / MCP / memory attach) and the
 * toolbar pills are registered before the user can interact, and so the pills
 * don't flash in after the composer paints. Registration is module-level +
 * one-shot, so this is a brief wait only on the first /chat visit of a session.
 */
export function useChatExtensionsReady(): boolean {
  const [ready, setReady] = useState(false)
  useEffect(() => {
    let active = true
    void chatExtensionsReady.then(() => {
      if (active) setReady(true)
    })
    return () => {
      active = false
    }
  }, [])
  return ready
}
