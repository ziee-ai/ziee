import { Button } from '@ziee/kit'
import { ArrowDown } from 'lucide-react'
import { cn } from '@/lib/utils'

interface JumpToLatestButtonProps {
  /** Shown only when the user has scrolled up from the latest message. */
  visible: boolean
  onClick: () => void
  className?: string
}

/**
 * JumpToLatestButton (ITEM-2) — a floating "scroll to latest" affordance that
 * appears only when the user is scrolled up (not at the bottom of the message
 * list) and returns them to the newest message on click. Visibility is driven
 * by the existing bottom-sentinel IntersectionObserver in ConversationPage
 * (DEC-12).
 */
export function JumpToLatestButton({
  visible,
  onClick,
  className,
}: JumpToLatestButtonProps) {
  if (!visible) return null
  return (
    <Button
      data-testid="chat-jump-to-latest-btn"
      variant="secondary"
      size="icon"
      icon={<ArrowDown />}
      onClick={onClick}
      aria-label="Jump to latest message"
      tooltip="Jump to latest"
      className={cn('rounded-full shadow-md border border-border', className)}
    />
  )
}
