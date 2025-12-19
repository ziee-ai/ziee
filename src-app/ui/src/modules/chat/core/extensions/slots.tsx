import { Fragment } from 'react'
import type { ChatSlotName } from '@/modules/chat/core/extensions/types'
import { chatExtensionRegistry } from '@/modules/chat/core/extensions/registry'

/**
 * Props for ExtensionSlot component
 */
interface ExtensionSlotProps {
  /** Name of the slot to render */
  name: ChatSlotName
  /** Optional wrapper className */
  className?: string
  /** Optional fallback content if no extensions render */
  fallback?: React.ReactNode
}

/**
 * Extension slot component
 * Renders all extension components registered for this slot
 * Extensions access Stores.Chat directly for conversation data
 */
export function ExtensionSlot({
  name,
  className,
  fallback,
}: ExtensionSlotProps) {
  const renderers = chatExtensionRegistry.renderSlot(name)

  if (renderers.length === 0) {
    return fallback ? <>{fallback}</> : null
  }

  return (
    <div className={className} data-chat-extension-slot={name}>
      {renderers.map((renderer, index) => (
        <Fragment key={`${name}-${index}`}>{renderer}</Fragment>
      ))}
    </div>
  )
}
