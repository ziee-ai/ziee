import { useEffect } from 'react'
import {
  Alert,
  Button,
  Card,
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
  summarize_after_n_messages: number
  summarizer_keep_recent: number
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
        summarize_after_n_messages: settings.summarize_after_n_messages,
        summarizer_keep_recent: settings.summarizer_keep_recent,
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
        summarize_after_n_messages: values.summarize_after_n_messages,
        summarizer_keep_recent: values.summarizer_keep_recent,
        full_summary_prompt: values.full_summary_prompt?.trim()
          ? values.full_summary_prompt
          : null,
        incremental_summary_prompt: values.incremental_summary_prompt?.trim()
          ? values.incremental_summary_prompt
          : null,
      })
      message.success('Summarizer settings saved.')
    } catch (e: any) {
      message.error(e?.message ?? 'Failed to save summarizer settings.')
    }
  }

  return (
    <Card title="Conversation summarizer">
      <Form
        name="memory-admin-summarizer-form"
        form={form}
        layout="vertical"
        onFinish={handleSubmit}
        disabled={!canManage}
      >
        <Form.Item
          name="summarize_after_n_messages"
          label="Summarize after N messages"
          extra="When a conversation branch exceeds this many user/assistant messages, the summarizer condenses the earliest ones into a single system block. Lower = sooner summarization (smaller prompts, more LLM cost); higher = longer verbatim history."
        >
          <InputNumber min={10} max={1000} />
        </Form.Item>
        <Form.Item
          name="summarizer_keep_recent"
          label="Keep recent messages verbatim"
          extra="How many of the most-recent messages stay unsummarized alongside the summary block. Must stay below the trigger above (DB-enforced)."
          dependencies={['summarize_after_n_messages']}
          rules={[
            ({ getFieldValue }) => ({
              validator(_, value) {
                const trigger = getFieldValue('summarize_after_n_messages')
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
          <InputNumber min={2} max={999} />
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
          <Flex justify="end">
            <Button type="primary" htmlType="submit" loading={saving}>
              Save
            </Button>
          </Flex>
        )}
      </Form>
    </Card>
  )
}
