import { Plus } from 'lucide-react'
import { Button } from '@ziee/kit'

interface AddButtonProps {
  onClick?: () => void
  /** Accessible name + tooltip (e.g. "Add server"). REQUIRED — the button is icon-only. */
  label: string
  disabled?: boolean
  /** Unique test selector (required — the kit Button enforces data-testid). */
  'data-testid': string
}

// The ONE convention for a list/card "add" affordance: a compact icon-only `+`
// PRIMARY (accent) button with a tooltip — deliberately prominent. Replaces the
// mix of bare `<Plus>` icons and "Add X" text buttons that varied per page.
export function AddButton({ onClick, label, disabled, 'data-testid': testid }: AddButtonProps) {
  return (
    <Button
      size="icon"
      variant="default"
      icon={<Plus aria-hidden="true" />}
      tooltip={label}
      aria-label={label}
      onClick={onClick}
      disabled={disabled}
      data-testid={testid}
    />
  )
}
