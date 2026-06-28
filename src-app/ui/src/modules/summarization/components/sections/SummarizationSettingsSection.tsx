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
  Select,
  Spin,
  Switch,
  message,
} from 'antd'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'

const READ_PERM = Permissions.SummarizationSettingsRead
const MANAGE_PERM = Permissions.SummarizationSettingsManage

interface FormValues {
  enabled: boolean
  default_summarization_model_id?: string | null
  summarize_after_tokens: number
  summarizer_keep_recent_tokens: number
  full_summary_prompt?: string | null
  incremental_summary_prompt?: string | null
}

/**
 * Deployment-wide summarization admin: enable toggle, summarizer model
 * (NULL → conversation's own model, zero-config), token thresholds,
 * prompt overrides. Mirrors the layout pattern shipped across all the
 * memory admin sections (horizontal labelCol/wrapperCol responsive).
 *
 * Per the audit lesson from the prior session's crashed implementation,
 * the section renders an explicit error state on load failure — not
 * a blank card — so an operator hitting a 5xx during settings load
 * sees what went wrong.
 */
export function SummarizationSettingsSection() {
  const canRead = usePermission(READ_PERM) || usePermission(MANAGE_PERM)
  const canManage = usePermission(MANAGE_PERM)
  const { settings, availableModels, loading, saving, loadingModels, error } =
    Stores.SummarizationAdmin
  const [form] = Form.useForm<FormValues>()

  useEffect(() => {
    if (settings) {
      form.setFieldsValue({
        enabled: settings.enabled,
        default_summarization_model_id: settings.default_summarization_model_id,
        summarize_after_tokens: settings.summarize_after_tokens,
        summarizer_keep_recent_tokens: settings.summarizer_keep_recent_tokens,
        full_summary_prompt: settings.full_summary_prompt,
        incremental_summary_prompt: settings.incremental_summary_prompt,
      })
    }
  }, [settings, form])

  if (!canRead) {
    return (
      <Card title="Summarization">
        <Alert
          type="warning"
          showIcon
          title="You don't have permission to view summarization settings."
        />
      </Card>
    )
  }

  // Audit lesson: render the error state, not a blank card.
  if (error && !settings) {
    return (
      <Card title="Summarization">
        <Alert
          type="error"
          showIcon
          title="Failed to load summarization settings"
          description={error}
        />
      </Card>
    )
  }

  if (loading && !settings) {
    return (
      <Card title="Summarization">
        <div className="flex justify-center py-4">
          <Spin />
        </div>
      </Card>
    )
  }
  if (!settings) return null

  const handleSubmit = async (values: FormValues) => {
    // Placeholder validation: a non-empty prompt override must contain
    // the placeholders the engine interpolates. Empty / null → reset
    // to compiled-in default.
    if (
      values.full_summary_prompt &&
      values.full_summary_prompt.trim() &&
      !values.full_summary_prompt.includes('{transcript}')
    ) {
      message.error(
        'Full summary prompt must contain the {transcript} placeholder.',
      )
      return
    }
    if (
      values.incremental_summary_prompt &&
      values.incremental_summary_prompt.trim() &&
      (!values.incremental_summary_prompt.includes('{previous_summary}') ||
        !values.incremental_summary_prompt.includes('{new_transcript}'))
    ) {
      message.error(
        'Incremental summary prompt must contain both {previous_summary} and {new_transcript} placeholders.',
      )
      return
    }
    if (values.summarizer_keep_recent_tokens >= values.summarize_after_tokens) {
      message.error(
        `Keep-recent (${values.summarizer_keep_recent_tokens}) must be less than the trigger (${values.summarize_after_tokens}).`,
      )
      return
    }
    try {
      await Stores.SummarizationAdmin.__state.update({
        enabled: values.enabled,
        default_summarization_model_id:
          values.default_summarization_model_id ?? null,
        summarize_after_tokens: values.summarize_after_tokens,
        summarizer_keep_recent_tokens: values.summarizer_keep_recent_tokens,
        full_summary_prompt:
          values.full_summary_prompt && values.full_summary_prompt.trim()
            ? values.full_summary_prompt
            : null,
        incremental_summary_prompt:
          values.incremental_summary_prompt &&
          values.incremental_summary_prompt.trim()
            ? values.incremental_summary_prompt
            : null,
      })
      message.success('Summarization settings saved.')
    } catch (e) {
      message.error(
        e instanceof Error ? e.message : 'Failed to save summarization settings.',
      )
    }
  }

  return (
    <Card title="Summarization">
      <Form
        name="summarization-admin-form"
        form={form}
        layout="horizontal"
        labelCol={{ xs: { span: 24 }, md: { span: 10 } }}
        wrapperCol={{ xs: { span: 24 }, md: { span: 14 } }}
        labelAlign="left"
        colon={false}
        onFinish={handleSubmit}
        disabled={!canManage}
      >
        <Form.Item
          name="enabled"
          label="Enable summarization"
          valuePropName="checked"
          extra="When off, no conversation is summarized regardless of length. Per-conversation toggle can still force it on (off-default) or off (on-default)."
        >
          <Switch aria-label="Enable summarization deployment-wide" />
        </Form.Item>

        <Form.Item
          name="default_summarization_model_id"
          label="Summarizer model"
          extra="LLM used to condense old turns into a summary. Leave empty to use the conversation's own model (zero-config; works out of the box on any deployment)."
        >
          <Select
            placeholder="Use the conversation's own model"
            loading={loadingModels}
            options={availableModels.map(m => ({
              value: m.id,
              label: m.display_name || m.name,
            }))}
            showSearch={{ optionFilterProp: 'label' }}
            allowClear
            style={{ maxWidth: 480 }}
          />
        </Form.Item>

        <Form.Item
          name="summarize_after_tokens"
          label="Summarize after N tokens"
          extra="Trigger threshold (estimated tokens, chars/4). Capped at 0.75× the chat model's context window so small-context local models summarize before they overflow."
          rules={[{ required: true }]}
        >
          <InputNumber min={500} max={1_000_000} step={500} style={{ width: 200 }} />
        </Form.Item>

        <Form.Item
          name="summarizer_keep_recent_tokens"
          label="Keep recent tokens verbatim"
          extra="Most-recent messages kept verbatim (not summarized). Must be less than the trigger."
          rules={[{ required: true }]}
        >
          <InputNumber
            min={100}
            max={1_000_000}
            step={500}
            style={{ width: 200 }}
          />
        </Form.Item>

        <Form.Item
          name="full_summary_prompt"
          label="Full-summary prompt"
          extra="Custom LLM prompt for the first summarization. Empty = use the compiled default. Must contain {transcript}."
        >
          <Input.TextArea autoSize={{ minRows: 2, maxRows: 6 }} />
        </Form.Item>

        <Form.Item
          name="incremental_summary_prompt"
          label="Incremental-summary prompt"
          extra="Custom LLM prompt for incremental folds. Empty = use the compiled default. Must contain {previous_summary} AND {new_transcript}."
        >
          <Input.TextArea autoSize={{ minRows: 2, maxRows: 6 }} />
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
