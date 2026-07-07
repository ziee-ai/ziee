import { useEffect } from 'react'
import {
  Alert,
  Card,
  ErrorState,
  Form,
  FormField,
  useForm,
  zodResolver,
  Textarea,
  InputNumber,
  Combobox,
  Spin,
  Switch,
  message,
} from '@/components/ui'
import { z } from 'zod'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'
import { SettingsFormActions } from '@/modules/settings/components/SettingsFormActions'

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

const schema = z.object({
  enabled: z.boolean(),
  default_summarization_model_id: z.string().nullable().optional(),
  summarize_after_tokens: z.number(),
  summarizer_keep_recent_tokens: z.number(),
  full_summary_prompt: z.string().nullable().optional(),
  incremental_summary_prompt: z.string().nullable().optional(),
})

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
  const form = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: {
      enabled: false,
      default_summarization_model_id: null,
      summarize_after_tokens: 8000,
      summarizer_keep_recent_tokens: 2000,
      full_summary_prompt: null,
      incremental_summary_prompt: null,
    },
  })

  useEffect(() => {
    if (settings) {
      form.reset({
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
      <Card title="Summarization" data-testid="summ-settings-noperm-card">
        <Alert
          tone="warning"
          data-testid="summ-settings-noperm-alert"
          title="You don't have permission to view summarization settings."
        />
      </Card>
    )
  }

  // Audit lesson: render the error state, not a blank card.
  if (error && !settings) {
    return (
      <Card title="Summarization" data-testid="summ-settings-error-card">
        <ErrorState
          resource="summarization settings"
          description="The summarization settings couldn't be loaded. Check your connection and try again."
          details={error}
          onRetry={() => void Stores.SummarizationAdmin.load()}
          data-testid="summ-settings-error"
        />
      </Card>
    )
  }

  if (loading && !settings) {
    return (
      <Card data-testid="summarization-settings-loading-card" title="Summarization">
        <div className="flex justify-center py-4">
          <Spin label="Loading summarization settings" />
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
      await Stores.SummarizationAdmin.update({
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
    <Card
      title="Summarization"
      data-testid="summ-settings-card"
      footer={canManage ? (
        <SettingsFormActions
          onSave={form.handleSubmit(handleSubmit)}
          onCancel={() => form.reset()}
          saving={saving}
          saveTestid="summ-save-button"
          cancelTestid="summ-cancel-button"
        />
      ) : undefined}
    >
      <Form
        data-testid="summ-settings-form"
        name="summarization-admin-form"
        form={form}
        layout="horizontal"
        onSubmit={handleSubmit}
        disabled={!canManage}
      >
        <FormField
          name="enabled"
          label="Enable summarization"
          valuePropName="checked"
          description="When off, no conversation is summarized regardless of length. Per-conversation toggle can still force it on (off-default) or off (on-default)."
        >
          <Switch data-testid="summ-enabled-switch" aria-label="Enable summarization deployment-wide" />
        </FormField>

        <FormField
          name="default_summarization_model_id"
          label="Summarizer model"
          description="LLM used to condense old turns into a summary. Leave empty to use the conversation's own model (zero-config; works out of the box on any deployment)."
        >
          <Combobox
            data-testid="summ-model-combobox"
            placeholder="Use the conversation's own model"
            searchPlaceholder="Search models"
            emptyText="No models found"
            loading={loadingModels}
            options={availableModels.map(m => ({
              value: m.id,
              label: m.display_name || m.name,
            }))}
            className="max-w-[480px]"
          />
        </FormField>

        <FormField
          name="summarize_after_tokens"
          label="Summarize after N tokens"
          description="Trigger threshold (estimated tokens, chars/4). Capped at 0.75× the chat model's context window so small-context local models summarize before they overflow."
          required
        >
          <InputNumber data-testid="summ-after-tokens-input" min={500} max={1_000_000} step={500} className="w-[200px]" />
        </FormField>

        <FormField
          name="summarizer_keep_recent_tokens"
          label="Keep recent tokens verbatim"
          description="Most-recent messages kept verbatim (not summarized). Must be less than the trigger."
          required
        >
          <InputNumber
            data-testid="summ-keep-recent-input"
            min={100}
            max={1_000_000}
            step={500}
            className="w-[200px]"
          />
        </FormField>

        <FormField
          name="full_summary_prompt"
          label="Full-summary prompt"
          description="Custom LLM prompt for the first summarization. Empty = use the compiled default. Must contain {transcript}."
        >
          <Textarea data-testid="summ-full-prompt-textarea" autoSize={{ minRows: 2, maxRows: 6 }} />
        </FormField>

        <FormField
          name="incremental_summary_prompt"
          label="Incremental-summary prompt"
          description="Custom LLM prompt for incremental folds. Empty = use the compiled default. Must contain {previous_summary} AND {new_transcript}."
        >
          <Textarea data-testid="summ-incremental-prompt-textarea" autoSize={{ minRows: 2, maxRows: 6 }} />
        </FormField>

      </Form>
    </Card>
  )
}
