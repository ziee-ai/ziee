import { Button, Flex } from '@/components/ui'

interface SettingsFormActionsProps {
  /** Primary submit handler — wire to `form.handleSubmit(onSubmit)`. */
  onSave: () => void
  onCancel: () => void
  saving?: boolean
  /** Disable cancel (e.g. while saving). */
  cancelDisabled?: boolean
  /** Disable the primary action (e.g. a blocking background job in progress). */
  saveDisabled?: boolean
  saveLabel?: string
  cancelLabel?: string
  /** Unique test selectors (required — the kit Button enforces data-testid). */
  saveTestid: string
  cancelTestid: string
}

// The ONE convention for a card's Save/Cancel actions. Rendered in the Card
// `footer` slot (never as a Separator + inline buttons in the body), with the
// secondary action as `outline` and the primary as the default (accent) button.
export function SettingsFormActions({
  onSave, onCancel, saving, cancelDisabled, saveDisabled,
  saveLabel = 'Save', cancelLabel = 'Cancel', saveTestid, cancelTestid,
}: SettingsFormActionsProps) {
  return (
    <Flex justify="end" gap="small" className="w-full">
      <Button
        type="button"
        variant="outline"
        onClick={onCancel}
        disabled={cancelDisabled ?? saving}
        data-testid={cancelTestid}
      >
        {cancelLabel}
      </Button>
      <Button type="button" loading={saving} disabled={saveDisabled} onClick={onSave} data-testid={saveTestid}>
        {saveLabel}
      </Button>
    </Flex>
  )
}
