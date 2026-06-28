import { Button, Input, PasswordInput, Switch, Tooltip, Text, FormField, FormList, useFormContext, useWatch, Flex } from '@/components/ui'
import { Trash2, Plus } from 'lucide-react'

/**
 * Form.List-based editor for repeating key/value entries with a
 * per-row "secret" toggle. Replaces the legacy JSON `TextArea`
 * editors for MCP server `environment_variables` and `headers`.
 *
 * Form state shape per row (read by McpServerDrawer's onSubmit):
 *
 *   {
 *     key: string,
 *     value: string | undefined,    // empty for saved secrets
 *     is_secret: boolean,
 *     _was_saved_secret: boolean,   // hidden — see notes
 *   }
 *
 * `_was_saved_secret` is set by the form initializer when an entry
 * came back from the server with `is_secret: true && value: null`
 * (the write-only-secret semantic). It controls the password input's
 * placeholder text: `••••• (saved)` vs the value-placeholder prop.
 * When the user submits without re-typing the value, the consumer
 * should send `value: null` to the API so the server keeps the
 * existing encrypted value.
 *
 * Behavior:
 * - Toggling `is_secret` from OFF→ON wipes the visible value so the
 *   admin doesn't accidentally save plaintext into the encrypted
 *   column. Conversely ON→OFF wipes any cached value so the user
 *   must explicitly re-type to keep it (avoids accidental decrypt-
 *   to-plain when the user only meant to "show me what's there").
 * - The remove (trash) button drops the whole row. If the row was a
 *   saved secret, this effectively clears it on the next save.
 */
interface KeyValueSecretEditorProps {
  /** Form path for the list field, e.g. `'environment_variables_entries'`. */
  name: string
  /** Default for new rows the user adds. Env vars usually carry secrets; non-Authorization headers usually don't. */
  defaultIsSecret: boolean
  keyPlaceholder: string
  valuePlaceholder: string
  /** "env var" | "header" — used in the Add button + empty-state text. */
  labelSingular: string
}

interface KeyValueRowProps {
  listName: string
  index: number
  remove: (index: number) => void
  keyPlaceholder: string
  valuePlaceholder: string
  labelSingular: string
}

// Extracted sub-component so that useWatch (a hook) can be called
// per-row inside the FormList render-prop callback.
function KeyValueRow({
  listName,
  index,
  remove,
  keyPlaceholder,
  valuePlaceholder,
  labelSingular,
}: KeyValueRowProps) {
  const { setValue } = useFormContext()
  const path = `${listName}.${index}`
  // Re-render only when this row's is_secret or _was_saved_secret
  // changes — avoids a full re-render on every keystroke in the
  // value field.
  const isSecret = useWatch({ name: `${path}.is_secret` })
  const wasSavedSecret = useWatch({ name: `${path}._was_saved_secret` })

  return (
    // Plain flex layout instead of Space.Compact — Compact wraps
    // each child in a sizing shell that swallows `flex` props on
    // the children, which made the rows un-responsive at narrow
    // drawer widths.
    <div className="flex flex-wrap items-start gap-2 w-full">
      <FormField
        name={`${path}.key`}
        aria-label={`${labelSingular} key`}
        required
        className="!mb-0 flex-1 min-w-40"
      >
        <Input
          placeholder={keyPlaceholder}
          className="font-mono text-xs"
          data-testid={`mcp-kv-${listName}-key-${index}`}
        />
      </FormField>
      <FormField
        name={`${path}.value`}
        aria-label={`${labelSingular} value`}
        className="!mb-0 flex-1 min-w-40"
      >
        {isSecret ? (
          <PasswordInput
            placeholder={
              wasSavedSecret
                ? '••••• (saved — leave blank to keep)'
                : valuePlaceholder
            }
            autoComplete="new-password"
            className="font-mono text-xs"
            showLabel="Show value"
            hideLabel="Hide value"
            data-testid={`mcp-kv-${listName}-value-${index}`}
          />
        ) : (
          <Input
            placeholder={valuePlaceholder}
            className="font-mono text-xs"
            data-testid={`mcp-kv-${listName}-value-${index}`}
          />
        )}
      </FormField>
      <Tooltip
        title={
          isSecret
            ? 'Secret — value is encrypted at rest and never returned to the client.'
            : 'Plain — value is stored as-is and visible in API responses.'
        }
      >
        <FormField
          name={`${path}.is_secret`}
          aria-label={`${labelSingular} is secret`}
          valuePropName="checked"
          className="!mb-0 shrink-0"
        >
          <Switch
            data-testid={`mcp-kv-${listName}-secret-${index}`}
            onChange={() => {
              // Toggling the switch wipes the value so
              // the user has to opt-in to whatever the
              // new storage shape is (don't silently
              // decrypt-to-plain or encrypt-the-shown-
              // value). Also clear the
              // _was_saved_secret marker — once the
              // user has touched the entry, the saved-
              // secret affordance is no longer accurate.
              setValue(`${path}.value`, '')
              setValue(`${path}._was_saved_secret`, false)
            }}
          />
        </FormField>
      </Tooltip>
      <Button
        size="icon"
        variant="ghost"
        tooltip={`Remove ${labelSingular}`}
        onClick={() => remove(index)}
        className="shrink-0"
        data-testid={`mcp-kv-${listName}-remove-${index}`}
      >
        <Trash2 className="h-4 w-4" />
      </Button>
    </div>
  )
}

export function KeyValueSecretEditor({
  name,
  defaultIsSecret,
  keyPlaceholder,
  valuePlaceholder,
  labelSingular,
}: KeyValueSecretEditorProps) {
  return (
    <FormList name={name}>
      {({ fields, append, remove }) => (
        <Flex vertical className="gap-2">
          {fields.length === 0 && (
            <Text type="secondary" className="text-xs">
              No {labelSingular}s configured.
            </Text>
          )}
          {fields.map((field, i) => (
            <KeyValueRow
              key={field.id}
              listName={name}
              index={i}
              remove={remove}
              keyPlaceholder={keyPlaceholder}
              valuePlaceholder={valuePlaceholder}
              labelSingular={labelSingular}
            />
          ))}
          <Button
            variant="outline"
            type="button"
            data-testid={`mcp-kv-${name}-add-btn`}
            onClick={() =>
              append({
                key: '',
                value: '',
                is_secret: defaultIsSecret,
                _was_saved_secret: false,
              })
            }
            icon={<Plus className="h-4 w-4" />}
          >
            Add {labelSingular}
          </Button>
        </Flex>
      )}
    </FormList>
  )
}
