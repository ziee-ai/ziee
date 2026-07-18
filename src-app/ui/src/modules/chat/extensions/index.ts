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

import { chatExtensionRegistry } from '@/modules/chat/core/extensions'
import type { ChatExtension } from '@/modules/chat/core/extensions'
import { collectGlobDefaults } from '@ziee/framework/slots'

// Path 1: in-chat extensions.
const inChatExtensions = import.meta.glob<{ default: ChatExtension }>(
  './*/extension.tsx',
  { eager: true },
)

// Path 2: sibling-module extensions at modules/<name>/chat-extension/extension.tsx.
// Relative pattern is required — `@/` alias isn't supported by import.meta.glob.
const siblingModuleExtensions = import.meta.glob<{ default: ChatExtension }>(
  '../../*/chat-extension/extension.tsx',
  { eager: true },
)

console.log('[Chat Extensions] Auto-discovering extensions...')

// Merge both glob maps + extract default exports via the generic
// auto-discovery helper (gap G8); the priority-ordering policy stays here.
const discovered = collectGlobDefaults<ChatExtension>(
  inChatExtensions,
  siblingModuleExtensions,
)

const discoveredExtensions: ChatExtension[] = []
for (const { path, value: extension } of discovered) {
  discoveredExtensions.push(extension)
  console.log(`[Chat Extensions] Discovered: ${extension.name} (${path})`)
}

// Sort extensions by priority (lower = higher priority)
const sortedExtensions = discoveredExtensions.sort(
  (a, b) => (a.priority ?? 100) - (b.priority ?? 100),
)

// Register extensions in priority order
for (const extension of sortedExtensions) {
  chatExtensionRegistry.register(extension, { enabled: true })
}

console.log(
  '[Chat Extensions] Registered',
  chatExtensionRegistry.getExtensions().length,
  'extensions:',
  sortedExtensions.map(ext => `${ext.name}(${ext.priority ?? 100})`).join(', '),
)

/**
 * Re-export extension registry for convenience
 */
export { chatExtensionRegistry } from '../core/extensions'
