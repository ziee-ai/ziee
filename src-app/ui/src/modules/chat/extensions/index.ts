/**
 * Chat Extensions Auto-Discovery
 *
 * This file automatically discovers and registers all chat extensions using Vite's
 * import.meta.glob() - the same pattern used by the module system.
 *
 * Extension Architecture:
 * - Each extension must be in its own directory: `extensions/[name]/extension.tsx`
 * - Export a default ChatExtension from extension.tsx
 * - Extensions are auto-discovered and registered at build time
 *
 * To add a new extension:
 * 1. Create a new directory under extensions/
 * 2. Create extension.tsx that exports a default ChatExtension
 * 3. The extension will be automatically discovered and registered
 *
 * Example extension structure:
 * ```typescript
 * // extensions/myextension/extension.tsx
 * import { createExtension, type ChatExtension } from '../../core/extensions'
 *
 * export default createExtension({
 *   name: 'myextension',
 *   description: 'My custom extension',
 *   priority: 50,
 *   actions: ['sse_event'],
 *   handleSSEEvent: async (event, context) => {
 *     // Handle custom events
 *     return { handled: false }
 *   }
 * })
 * ```
 *
 * Usage:
 * ```typescript
 * import '@/modules/chat/extensions'
 * ```
 */

import { chatExtensionRegistry } from '@/modules/chat/core/extensions'
import type { ChatExtension } from '@/modules/chat/core/extensions'

/**
 * Auto-discover all extension.tsx files in subdirectories
 */
const extensionFiles = import.meta.glob<{ default: ChatExtension }>(
  './*/extension.tsx',
  { eager: true },
)

/**
 * Register all discovered extensions
 */
console.log('[Chat Extensions] Auto-discovering extensions...')

const discoveredExtensions: ChatExtension[] = []

for (const [path, moduleExports] of Object.entries(extensionFiles)) {
  const extension = moduleExports.default
  if (extension) {
    discoveredExtensions.push(extension)
    console.log(`[Chat Extensions] Discovered: ${extension.name} (${path})`)
  }
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
