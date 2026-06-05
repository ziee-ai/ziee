import {
  Button,
  Flex,
  Form,
  Input,
  Space,
  Switch,
  Tooltip,
  Typography,
} from 'antd'
import {
  DeleteOutlined,
  KeyOutlined,
  LockOutlined,
  PlusOutlined,
} from '@ant-design/icons'

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
            <Space.Compact key={field.key} className="w-full" block>
              <Form.Item
                name={[field.name, 'key']}
                rules={[{ required: true, message: 'key required' }]}
                noStyle
              >
                <Input
                  placeholder={keyPlaceholder}
                  className="font-mono text-xs"
                  style={{ flex: '0 0 220px' }}
                />
              </Form.Item>
              <Form.Item shouldUpdate noStyle>
                {({ getFieldValue, setFieldValue }) => {
                  const path = [name, field.name]
                  const isSecret = getFieldValue([...path, 'is_secret'])
                  const wasSavedSecret = getFieldValue([
                    ...path,
                    '_was_saved_secret',
                  ])
                  return (
                    <>
                      <Form.Item name={[field.name, 'value']} noStyle>
                        {isSecret ? (
                          <Input.Password
                            placeholder={
                              wasSavedSecret
                                ? '••••• (saved — leave blank to keep)'
                                : valuePlaceholder
                            }
                            autoComplete="new-password"
                            className="font-mono text-xs"
                            style={{ flex: 1 }}
                          />
                        ) : (
                          <Input
                            placeholder={valuePlaceholder}
                            className="font-mono text-xs"
                            style={{ flex: 1 }}
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
                          noStyle
                        >
                          <Switch
                            checkedChildren={<LockOutlined />}
                            unCheckedChildren={<KeyOutlined />}
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
              <Form.Item
                name={[field.name, '_was_saved_secret']}
                hidden
                initialValue={false}
              >
                <Input type="hidden" />
              </Form.Item>
              <Button
                icon={<DeleteOutlined />}
                onClick={() => remove(field.name)}
                aria-label={`Remove ${labelSingular}`}
              />
            </Space.Compact>
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
            icon={<PlusOutlined />}
          >
            Add {labelSingular}
          </Button>
        </Flex>
      )}
    </Form.List>
  )
}
