import { useEffect } from 'react'
import {
  Alert,
  Button,
  Card,
  Divider,
  Flex,
  Form,
  Input,
  InputNumber,
  message,
} from 'antd'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'

const READ_PERM = Permissions.MemoryAdminRead
const MANAGE_PERM = Permissions.MemoryAdminManage

interface FormValues {
  summarize_after_tokens: number
  summarizer_keep_recent_tokens: number
  full_summary_prompt?: string
  incremental_summary_prompt?: string
}

/**
 * Conversation summarizer thresholds + LLM prompt overrides. Own
 * form so saving prompts doesn't touch the embedding model or
 * retrieval tuning.
 */
export function SummarizerSection() {
  const canRead = usePermission(READ_PERM) || usePermission(MANAGE_PERM)
  const canManage = usePermission(MANAGE_PERM)
  const { settings, saving } = Stores.MemoryAdmin
  const [form] = Form.useForm<FormValues>()

  useEffect(() => {
    if (settings) {
      form.setFieldsValue({
        summarize_after_tokens: settings.summarize_after_tokens,
        summarizer_keep_recent_tokens: settings.summarizer_keep_recent_tokens,
        full_summary_prompt: settings.full_summary_prompt ?? '',
        incremental_summary_prompt: settings.incremental_summary_prompt ?? '',
      })
    }
  }, [settings, form])

  if (!canRead) {
    return (
      <Card title="Conversation summarizer">
        <Alert
          type="warning"
          showIcon
          title="You don't have permission to view memory admin settings."
        />
      </Card>
    )
  }
  if (!settings) return null

  const handleSubmit = async (values: FormValues) => {
    try {
      await Stores.MemoryAdmin.update({
        summarize_after_tokens: values.summarize_after_tokens,
        summarizer_keep_recent_tokens: values.summarizer_keep_recent_tokens,
        full_summary_prompt: values.full_summary_prompt?.trim()
          ? values.full_summary_prompt
          : null,
        incremental_summary_prompt: values.incremental_summary_prompt?.trim()
          ? values.incremental_summary_prompt
          : null,
      })
      message.success('Summarizer settings saved.')
    } catch (error) {
      message.error(
        error instanceof Error
          ? error.message
          : 'Failed to save summarizer settings.',
      )
    }
  }

  return (
    <Card title="Conversation summarizer">
      <Form
        name="memory-admin-summarizer-form"
        form={form}
        layout="horizontal"
        labelCol={{ flex: '280px' }}
        wrapperCol={{ flex: 'auto' }}
        labelAlign="left"
        colon={false}
        onFinish={handleSubmit}
        disabled={!canManage}
      >
        <Form.Item
          name="summarize_after_tokens"
          label="Summarize after N tokens"
          extra="When a conversation branch's estimated tokens (chars/4) exceed this, the summarizer condenses the earliest messages into a single system block. Token-aware (a 5-token message and a 50K-token one are not equal). Lower = sooner summarization (smaller prompts, more LLM cost); higher = longer verbatim history."
        >
          <InputNumber
            min={500}
            max={1000000}
            step={1000}
            style={{ width: 160 }}
          />
        </Form.Item>
        <Form.Item
          name="summarizer_keep_recent_tokens"
          label="Keep recent tokens verbatim"
          extra="Estimated tokens of the most-recent messages kept unsummarized alongside the summary block. The cutoff snaps to a message boundary. Must stay below the trigger above (DB-enforced)."
          dependencies={['summarize_after_tokens']}
          rules={[
            ({ getFieldValue }) => ({
              validator(_, value) {
                const trigger = getFieldValue('summarize_after_tokens')
                if (value == null || trigger == null || value < trigger) {
                  return Promise.resolve()
                }
                return Promise.reject(
                  new Error(
                    `Keep-recent (${value}) must be less than the trigger (${trigger}).`,
                  ),
                )
              },
            }),
          ]}
        >
          <InputNumber
            min={100}
            max={999999}
            step={500}
            style={{ width: 160 }}
          />
        </Form.Item>

        <Form.Item
          name="full_summary_prompt"
          label="Full-summarize LLM prompt"
          extra={
            <>
              Prompt sent to the summarization LLM the first time a
              branch is summarized (or when the incremental anchor is
              lost). Must contain the <code>{'{transcript}'}</code>{' '}
              placeholder. Leave empty to use the built-in default.
            </>
          }
          rules={[
            {
              validator(_, value: string | undefined) {
                const v = value?.trim() ?? ''
                if (v === '' || v.includes('{transcript}')) {
                  return Promise.resolve()
                }
                return Promise.reject(
                  new Error('Must contain the {transcript} placeholder.'),
                )
              },
            },
          ]}
        >
          <Input.TextArea
            autoSize={{ minRows: 4, maxRows: 14 }}
            placeholder="Leave empty to use the built-in default (recommended unless you have a specific reason to tune)."
          />
        </Form.Item>

        <Form.Item
          name="incremental_summary_prompt"
          label="Incremental-refresh LLM prompt"
          extra={
            <>
              Prompt for the incremental refresh path (every subsequent
              summarization fold-in). Must contain both{' '}
              <code>{'{previous_summary}'}</code> and{' '}
              <code>{'{new_transcript}'}</code> placeholders. Leave
              empty to use the built-in default.
            </>
          }
          rules={[
            {
              validator(_, value: string | undefined) {
                const v = value?.trim() ?? ''
                if (
                  v === '' ||
                  (v.includes('{previous_summary}') &&
                    v.includes('{new_transcript}'))
                ) {
                  return Promise.resolve()
                }
                return Promise.reject(
                  new Error(
                    'Must contain both {previous_summary} and {new_transcript} placeholders.',
                  ),
                )
              },
            },
          ]}
        >
          <Input.TextArea
            autoSize={{ minRows: 4, maxRows: 14 }}
            placeholder="Leave empty to use the built-in default."
          />
        </Form.Item>

        {canManage && (
          <>
            <Divider className="!my-3" />
            <Flex justify="end">
              <Button type="primary" htmlType="submit" loading={saving}>
                Save
              </Button>
            </Flex>
          </>
        )}
      </Form>
    </Card>
  )
}
