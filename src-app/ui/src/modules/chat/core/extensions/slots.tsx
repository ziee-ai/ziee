import { createExtensionSlot } from '@ziee/framework/slots'
import type { ChatSlotName } from '@/modules/chat/core/extensions/types'
import { chatExtensionRegistry } from '@/modules/chat/core/extensions/registry'

/**
 * Extension slot component.
 *
 * Built on the generic `@ziee/framework/slots` `createExtensionSlot` factory
 * (gap G8), bound to the chat extension registry (which delegates `renderSlot`
 * to the same generic slot registry). Renders all extension components
 * registered for a slot; extensions access `Chat` directly for data.
 *
 * The `data-chat-extension-slot` wrapper attribute is preserved byte-for-byte
 * so existing DOM + E2E selectors are unchanged.
 */
export const ExtensionSlot = createExtensionSlot<ChatSlotName>(
  chatExtensionRegistry,
  { slotAttr: 'data-chat-extension-slot' },
)
