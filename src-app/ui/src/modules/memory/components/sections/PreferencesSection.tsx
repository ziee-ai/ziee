import { useEffect } from 'react'
import {
  Alert,
  Button,
  Card,
  ErrorState,
  Flex,
  Form,
  FormField,
  useForm,
  zodResolver,
  InputNumber,
  Spin,
  Switch,
  message,
} from '@ziee/kit'
import { z } from 'zod'
import { Stores } from '@ziee/framework/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/permissions'
import { SettingsFormActions } from '@/modules/settings/components/SettingsFormActions'


const READ_PERM = Permissions.MemoryRead
const WRITE_PERM = Permissions.MemoryWrite

// Retention control: the days InputNumber and its "Forever" (clear = null) escape
// hatch rendered as ONE grouped row instead of a Forever button orphaned on a
// separate right-aligned line. It's the single child FormField clones, so it
// receives the injected value/onChange binding and forwards it to the input;
// "Forever" clears the value the same way typing an empty field would.
function RetentionControl({
  value,
  onChange,
  ...rest
}: {
  value?: number | null
  onChange?: (v: number | null) => void
} & Record<string, unknown>) {
  return (
    <Flex align="center" gap="small">
      <InputNumber
        {...rest}
        value={value ?? undefined}
        onChange={v => onChange?.(v as number | null)}
        min={1}
        max={3650}
        suffix="days"
        className="w-40"
        // This IS a form field — it's rendered as the single child of a
        // <FormField> at the call site (which owns the label + Field gap). The
        // settings-field linter can't see through this component boundary
        // (it "can't introspect JSX ancestry"), so flag the known blind spot.
        data-standalone-control
        data-testid="memory-prefs-retention-input"
      />
      <Button
        variant="outline"
        onClick={() => onChange?.(null)}
        data-testid="memory-prefs-forever-btn"
      >
        Forever
      </Button>
    </Flex>
  )
}

const schema = z.object({
  extraction_enabled: z.boolean(),
  retrieval_enabled: z.boolean(),
  max_memories: z.number().min(1).max(100000),
  retention_days: z.number().min(1).max(3650).nullable(),
})
type FormValues = z.infer<typeof schema>

/**
 * Per-user memory preferences: extraction/retrieval toggles + storage caps.
 *
 * Hidden entirely if the viewer doesn't have `memory::read`. The page
 * itself is gated on the `MEMORY_USER_READ_PERM` anyOf, so a user
 * with only `memory::core::read` reaches the page — but this section
 * is skipped because the underlying preferences are owned by the
 * vector-memory subsystem the user doesn't have access to.
 */
export function PreferencesSection() {
  const canRead = usePermission(READ_PERM)
  const canWrite = usePermission(WRITE_PERM)
  const { settings, loading, saving, error } = Stores.MemorySettings
  const { settings: adminSettings } = Stores.MemoryAdmin
  const form = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: {
      extraction_enabled: false,
      retrieval_enabled: false,
      max_memories: 1000,
      retention_days: null,
    },
  })

  useEffect(() => {
    if (settings) {
      form.reset({
        extraction_enabled: settings.extraction_enabled,
        retrieval_enabled: settings.retrieval_enabled,
        max_memories: settings.max_memories,
        retention_days: settings.retention_days,
      })
    }
  }, [settings, form])

  if (!canRead) return null

  const adminDisabled = adminSettings && !adminSettings.enabled

  if (error && !settings) {
    return (
      <Card title="Preferences" data-testid="memory-prefs-card">
        <ErrorState
          resource="memory preferences"
          description="Something went wrong while loading your memory preferences."
          details={error}
          onRetry={() => Stores.MemorySettings.load()}
          data-testid="memory-prefs-error"
        />
      </Card>
    )
  }

  if (loading || !settings) {
    return (
      <Card title="Preferences" data-testid="memory-prefs-card">
        <div className="flex justify-center py-6">
          <Spin label="Loading" />
        </div>
      </Card>
    )
  }

  const handleSubmit = async (values: FormValues) => {
    try {
      await Stores.MemorySettings.update({
        extraction_enabled: values.extraction_enabled,
        retrieval_enabled: values.retrieval_enabled,
        max_memories: values.max_memories,
        retention_days: values.retention_days ?? null,
      })
      message.success('Preferences saved.')
    } catch (error) {
      message.error(
        error instanceof Error ? error.message : 'Failed to save preferences.',
      )
    }
  }

  return (
    <>
      {adminDisabled && (
        <Alert
          tone="warning"
          title="Memory is currently disabled by the administrator."
          description="Settings here will be saved but have no effect until the administrator enables memory."
          data-testid="memory-prefs-admin-disabled-alert"
        />
      )}
      <Card
        title="Preferences"
        data-testid="memory-prefs-card"
        footer={canWrite ? (
          <SettingsFormActions
            onSave={form.handleSubmit(handleSubmit)}
            onCancel={() => form.reset({
              extraction_enabled: settings.extraction_enabled,
              retrieval_enabled: settings.retrieval_enabled,
              max_memories: settings.max_memories,
              retention_days: settings.retention_days,
            })}
            saving={saving}
            saveTestid="memory-prefs-save-btn"
            cancelTestid="memory-prefs-cancel-btn"
          />
        ) : undefined}
      >
        {/*
        Horizontal layout: label + description on the left, the
        control on the right. Compact enough that Switch /
        InputNumber controls don't drown in vertical whitespace.
      */}
      <Form
        name="memory-preferences-form"
        form={form}
        layout="horizontal"
        onSubmit={handleSubmit}
        disabled={!canWrite}
        data-testid="memory-prefs-form"
      >
        <FormField
          name="extraction_enabled"
          label="Auto-extract memories"
          valuePropName="checked"
          description="After each assistant reply, an LLM scans your turn for durable facts about you and stores them."
        >
          <Switch data-testid="memory-prefs-extraction-switch" />
        </FormField>
        <FormField
          name="retrieval_enabled"
          label="Inject relevant memories"
          valuePropName="checked"
          description="Before each LLM call, your latest message is embedded and the top-K most-similar memories are added to the system prompt."
        >
          <Switch data-testid="memory-prefs-retrieval-switch" />
        </FormField>
        <FormField
          name="max_memories"
          label="Max memories stored"
          description="When this cap is reached the reaper soft-deletes the oldest."
        >
          <InputNumber min={1} max={100000} className="w-40" data-testid="memory-prefs-max-input" />
        </FormField>
        <FormField
          name="retention_days"
          label="Retention"
          description="Empty = forever. Older memories are soft-deleted by the nightly reaper."
        >
          <RetentionControl />
        </FormField>
      </Form>
      </Card>
    </>
  )
}
