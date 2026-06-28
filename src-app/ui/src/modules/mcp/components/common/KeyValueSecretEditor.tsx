import { Button, Flex, Form, Input, Switch, Tooltip, Typography } from 'antd'
import {
  Trash2,
  Key,
  Lock,
  Plus,
} from 'lucide-react'

const { Text } = Typography

/**
 * Form.List-based editor for repeating key/value entries with a
 * per-row "secret" toggle. Replaces the legacy JSON `TextArea`
 * editors for MCP server `environment_variables` and `headers`.
 *
 * Form state shape per row (read by McpServerDrawer's onFinish):
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
  /** antd Form path for the list field, e.g. `'environment_variables_entries'`. */
  name: string
  /** Default for new rows the user adds. Env vars usually carry secrets; non-Authorization headers usually don't. */
  defaultIsSecret: boolean
  keyPlaceholder: string
  valuePlaceholder: string
  /** "env var" | "header" — used in the Add button + empty-state text. */
  labelSingular: string
}

export function KeyValueSecretEditor({
  name,
  defaultIsSecret,
  keyPlaceholder,
  valuePlaceholder,
  labelSingular,
}: KeyValueSecretEditorProps) {
  return (
    <Form.List name={name}>
      {(fields, { add, remove }) => (
        <Flex vertical className="gap-2">
          {fields.length === 0 && (
            <Text type="secondary" className="text-xs">
              No {labelSingular}s configured.
            </Text>
          )}
          {fields.map(field => (
            // Plain flex layout instead of Space.Compact — Compact
            // wraps each child in a sizing shell that swallows
            // `flex` props on the children, which made the rows
            // un-responsive at narrow drawer widths (key column
            // pinned to 220px regardless, value column overflowing).
            // Hidden _was_saved_secret rendered as a visible empty
            // cell inside the compact group too.
            <Flex
              key={field.key}
              className="w-full"
              align="start"
              gap="small"
              wrap
            >
              <Form.Item
                name={[field.name, 'key']}
                rules={[{ required: true, message: 'key required' }]}
                className="!mb-0 flex-1 min-w-40"
              >
                <Input
                  placeholder={keyPlaceholder}
                  className="font-mono text-xs"
                />
              </Form.Item>
              <Form.Item
                noStyle
                shouldUpdate={(prev, curr) => {
                  // Re-render only when this row's is_secret or
                  // _was_saved_secret changes — avoids a full
                  // re-render on every keystroke in the value field.
                  const a = prev?.[name]?.[field.name]
                  const b = curr?.[name]?.[field.name]
                  return (
                    a?.is_secret !== b?.is_secret ||
                    a?._was_saved_secret !== b?._was_saved_secret
                  )
                }}
              >
                {({ getFieldValue, setFieldValue }) => {
                  const path = [name, field.name]
                  const isSecret = getFieldValue([...path, 'is_secret'])
                  const wasSavedSecret = getFieldValue([
                    ...path,
                    '_was_saved_secret',
                  ])
                  return (
                    <>
                      <Form.Item
                        name={[field.name, 'value']}
                        className="!mb-0 flex-1 min-w-40"
                      >
                        {isSecret ? (
                          <Input.Password
                            placeholder={
                              wasSavedSecret
                                ? '••••• (saved — leave blank to keep)'
                                : valuePlaceholder
                            }
                            autoComplete="new-password"
                            className="font-mono text-xs"
                          />
                        ) : (
                          <Input
                            placeholder={valuePlaceholder}
                            className="font-mono text-xs"
                          />
                        )}
                      </Form.Item>
                      <Tooltip
                        title={
                          isSecret
                            ? 'Secret — value is encrypted at rest and never returned to the client.'
                            : 'Plain — value is stored as-is and visible in API responses.'
                        }
                      >
                        <Form.Item
                          name={[field.name, 'is_secret']}
                          valuePropName="checked"
                          className="!mb-0 shrink-0"
                        >
                          <Switch
                            checkedChildren={<Lock />}
                            unCheckedChildren={<Key />}
                            onChange={() => {
                              // Toggling the switch wipes the value so
                              // the user has to opt-in to whatever the
                              // new storage shape is (don't silently
                              // decrypt-to-plain or encrypt-the-shown-
                              // value). Also clear the
                              // _was_saved_secret marker — once the
                              // user has touched the entry, the saved-
                              // secret affordance is no longer
                              // accurate.
                              setFieldValue([...path, 'value'], '')
                              setFieldValue(
                                [...path, '_was_saved_secret'],
                                false,
                              )
                            }}
                          />
                        </Form.Item>
                      </Tooltip>
                    </>
                  )
                }}
              </Form.Item>
              <Button
                icon={<Trash2 />}
                onClick={() => remove(field.name)}
                aria-label={`Remove ${labelSingular}`}
                className="shrink-0"
              />
              {/* Hidden field for the saved-secret marker — rendered
                  outside the Flex row so it doesn't take a layout
                  cell. `display: none` keeps the Form.Item registered
                  with the form state without occupying space. */}
              <Form.Item
                name={[field.name, '_was_saved_secret']}
                initialValue={false}
                hidden
                className="!hidden"
              >
                <Input type="hidden" />
              </Form.Item>
            </Flex>
          ))}
          <Button
            type="dashed"
            onClick={() =>
              add({
                key: '',
                value: '',
                is_secret: defaultIsSecret,
                _was_saved_secret: false,
              })
            }
            block
            icon={<Plus />}
          >
            Add {labelSingular}
          </Button>
        </Flex>
      )}
    </Form.List>
  )
}
