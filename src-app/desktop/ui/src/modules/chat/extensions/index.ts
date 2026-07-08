// DESKTOP override of the chat-extension auto-discovery index.
//
// The desktop app aliases `@/*` to the web-ui core (`ui/src/*`), with the
// localOverridePlugin resolving `desktop/ui/src` FIRST. The core discovery lives
// in `ui/src/modules/chat/extensions/index.ts`, and its sibling-extension glob is
// evaluated relative to THAT file's location — i.e. rooted at `ui/src/modules/`.
// So it can only ever see WEB-UI sibling extensions; a desktop-only module
// (office_bridge, at `desktop/ui/src/modules/office-bridge/`) is invisible to it.
//
// This override shadows the core index (desktop-first), runs the core discovery
// verbatim (so every web-ui extension still registers), and THEN globs the
// desktop-local sibling extensions (rooted here at `desktop/ui/src/modules/`) and
// registers them into the same shared `chatExtensionRegistry` singleton. Net: web
// extensions + desktop-only extensions, no registry edits in core (glob-driven,
// consistent with the web pattern).

import { chatExtensionRegistry } from '@/modules/chat/core/extensions'
import type { ChatExtension } from '@/modules/chat/core/extensions'

// Run the core web-ui discovery + registration (in-chat + web sibling modules).
// Explicit relative path (not `@/`) so this loads the CORE file, not itself.
import '../../../../../../ui/src/modules/chat/extensions/index'

// Desktop-only sibling extensions: rooted at `desktop/ui/src/modules/` here.
const desktopSiblingExtensions = import.meta.glob<{ default: ChatExtension }>(
  '../../*/chat-extension/extension.tsx',
  { eager: true },
)

const desktopExtensions: ChatExtension[] = []
for (const [path, mod] of Object.entries(desktopSiblingExtensions)) {
  if (mod.default) {
    desktopExtensions.push(mod.default)
    console.log(`[Chat Extensions] Discovered (desktop): ${mod.default.name} (${path})`)
  }
}

for (const ext of desktopExtensions.sort(
  (a, b) => (a.priority ?? 100) - (b.priority ?? 100),
)) {
  chatExtensionRegistry.register(ext, { enabled: true })
}

export { chatExtensionRegistry } from '@/modules/chat/core/extensions'
