import { Plus } from 'lucide-react'
import { Button } from '@ziee/kit'

interface AddButtonProps {
  onClick?: () => void
  /** Hover/focus intent — wire the lazy create action's `.preload()` here so its
   *  chunk is fetched before the user opens the drawer (the prefetch pattern). */
  onMouseEnter?: () => void
  /** Accessible name + tooltip (e.g. "Add server"). REQUIRED — the button is icon-only. */
  label: string
  disabled?: boolean
  /** Unique test selector (required — the kit Button enforces data-testid). */
  'data-testid': string
}

// The ONE convention for a list/card "add" affordance: a compact icon-only `+`
// PRIMARY (accent) button with a tooltip — deliberately prominent. Replaces the
// mix of bare `<Plus>` icons and "Add X" text buttons that varied per page.
export function AddButton({ onClick, onMouseEnter, label, disabled, 'data-testid': testid }: AddButtonProps) {
  return (
    <Button
      size="icon"
      variant="default"
      icon={<Plus aria-hidden="true" />}
      tooltip={label}
      aria-label={label}
      onClick={onClick}
      onMouseEnter={onMouseEnter}
      disabled={disabled}
      data-testid={testid}
    />
  )
}
