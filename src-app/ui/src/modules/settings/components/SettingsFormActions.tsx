import { Button, Flex, Tooltip } from '@/components/ui'

interface SettingsFormActionsProps {
  /** Primary submit handler — wire to `form.handleSubmit(onSubmit)`. */
  onSave: () => void
  onCancel: () => void
  saving?: boolean
  /** Disable cancel (e.g. while saving). */
  cancelDisabled?: boolean
  /** Disable the primary action (e.g. a blocking background job in progress). */
  saveDisabled?: boolean
  /** When the Save is disabled, the reason to surface on hover/focus. The Save
   *  stays the saturated primary variant either way (Spec B: "Save ALWAYS
   *  saturated primary; disabled needs a tooltip"), so a greyed Save always
   *  explains itself instead of reading as a broken/weak button. */
  saveDisabledReason?: string
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
  onSave, onCancel, saving, cancelDisabled, saveDisabled, saveDisabledReason,
  saveLabel = 'Save', cancelLabel = 'Cancel', saveTestid, cancelTestid,
}: SettingsFormActionsProps) {
  const saveButton = (
    <Button type="button" loading={saving} disabled={saveDisabled} onClick={onSave} data-testid={saveTestid}>
      {saveLabel}
    </Button>
  )
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
      {/* A disabled <button> swallows pointer events, so the reason-tooltip must
          attach to a focusable span wrapping it — the Save itself stays the
          saturated primary (Spec B). */}
      {saveDisabled && saveDisabledReason ? (
        <Tooltip title={saveDisabledReason}>
          <span
            tabIndex={0}
            aria-label={saveDisabledReason}
            className="inline-flex"
            data-testid={`${saveTestid}-disabled-wrap`}
          >
            {saveButton}
          </span>
        </Tooltip>
      ) : (
        saveButton
      )}
    </Flex>
  )
}
