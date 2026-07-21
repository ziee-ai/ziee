import { useEffect } from 'react'
import { z } from 'zod'
import { Permissions } from '@/api-client/permissions'
import {
  Alert,
  Card,
  ErrorState,
  Form,
  FormField,
  Input,
  InputNumber,
  message,
  Select,
  Separator,
  Spin,
  Switch,
  Text,
  useForm,
  zodResolver,
} from '@ziee/kit'
import { usePermission } from '@/core/permissions'
import { SettingsFormActions } from '@/modules/settings/components/SettingsFormActions'
import { VoiceConfig } from '@/modules/voice/stores/voiceConfig'
import { VoiceModel } from '@/modules/voice/stores/voiceModel'

const MIB = 1024 * 1024

const MODEL_OPTIONS = [
  { value: 'tiny', label: 'tiny (fastest, lowest accuracy)' },
  { value: 'base', label: 'base (balanced)' },
  { value: 'base.en', label: 'base.en (English-only)' },
  { value: 'small', label: 'small (best accuracy, slower)' },
]

const LANGUAGE_OPTIONS = [
  { value: 'auto', label: 'Auto-detect' },
  { value: 'en', label: 'English' },
  { value: 'es', label: 'Spanish' },
  { value: 'fr', label: 'French' },
  { value: 'de', label: 'German' },
  { value: 'it', label: 'Italian' },
  { value: 'pt', label: 'Portuguese' },
  { value: 'nl', label: 'Dutch' },
  { value: 'ja', label: 'Japanese' },
  { value: 'zh', label: 'Chinese' },
  { value: 'ko', label: 'Korean' },
  { value: 'ru', label: 'Russian' },
]

const schema = z.object({
  enabled: z.boolean(),
  model: z.string().min(1),
  model_source_repo: z.string().min(1),
  language: z.string().min(1),
  streaming_enabled: z.boolean(),
  stream_interval_ms: z.number().min(300).max(10000),
  stream_max_decode_secs: z.number().min(5).max(600),
  idle_unload_secs: z.number().min(0).max(86400),
  auto_start_timeout_secs: z.number().min(1).max(600),
  drain_timeout_secs: z.number().min(1).max(600),
  max_clip_seconds: z.number().min(1).max(600),
  max_upload_mib: z.number().min(1).max(200),
})
type Schema = z.infer<typeof schema>

/**
 * Deployment-wide voice settings: enable toggle, model + language, engine
 * timeouts, and the record/upload caps. Mirrors the peer settings-card layout.
 */
export function VoiceConfigCard() {
  const { settings, loadingSettings, savingSettings, error } =
    VoiceConfig
  const { installed } = VoiceModel
  const canManage = usePermission(Permissions.VoiceAdminManage)

  // The active-model options come from the INSTALLED library (so a downloaded/
  // uploaded model like `large-v3` is selectable), not a hardcoded list. The
  // currently-configured model is always included even if its file was removed,
  // and the standard names are offered as a fallback before anything is installed.
  const modelOptions = (() => {
    const opts = installed.length
      ? installed.map(m => ({
          value: m.name,
          label: m.verified ? m.name : `${m.name} (unverified)`,
        }))
      : [...MODEL_OPTIONS]
    const current = settings?.model
    if (current && !opts.some(o => o.value === current)) {
      opts.unshift({ value: current, label: current })
    }
    return opts
  })()

  const form = useForm<Schema>({
    resolver: zodResolver(schema),
    defaultValues: {
      enabled: false,
      model: 'base',
      model_source_repo: 'ggerganov/whisper.cpp',
      language: 'auto',
      streaming_enabled: true,
      stream_interval_ms: 1000,
      stream_max_decode_secs: 30,
      idle_unload_secs: 300,
      auto_start_timeout_secs: 30,
      drain_timeout_secs: 30,
      max_clip_seconds: 60,
      max_upload_mib: 25,
    },
  })

  useEffect(() => {
    if (settings) {
      form.reset({
        enabled: settings.enabled,
        model: settings.model,
        model_source_repo: settings.model_source_repo,
        language: settings.language,
        streaming_enabled: settings.streaming_enabled,
        stream_interval_ms: settings.stream_interval_ms,
        stream_max_decode_secs: settings.stream_max_decode_secs,
        idle_unload_secs: settings.idle_unload_secs,
        auto_start_timeout_secs: settings.auto_start_timeout_secs,
        drain_timeout_secs: settings.drain_timeout_secs,
        max_clip_seconds: settings.max_clip_seconds,
        max_upload_mib: Math.max(
          1,
          Math.round(settings.max_upload_bytes / MIB),
        ),
      })
    }
  }, [settings, form])

  const handleSave = async (values: Schema) => {
    try {
      await VoiceConfig.saveSettings({
        enabled: values.enabled,
        model: values.model,
        model_source_repo: values.model_source_repo,
        language: values.language,
        streaming_enabled: values.streaming_enabled,
        stream_interval_ms: values.stream_interval_ms,
        stream_max_decode_secs: values.stream_max_decode_secs,
        idle_unload_secs: values.idle_unload_secs,
        auto_start_timeout_secs: values.auto_start_timeout_secs,
        drain_timeout_secs: values.drain_timeout_secs,
        max_clip_seconds: values.max_clip_seconds,
        max_upload_bytes: values.max_upload_mib * MIB,
      })
      form.reset(values)
      message.success('Voice settings saved')
    } catch (e) {
      message.error(
        e instanceof Error ? e.message : 'Failed to save voice settings',
      )
    }
  }

  if (loadingSettings && !settings) {
    return (
      <Card title="Voice configuration" data-testid="voice-config-card">
        <Spin label="Loading" />
      </Card>
    )
  }

  if (error && !settings) {
    return (
      <Card title="Voice configuration" data-testid="voice-config-card">
        <ErrorState
          resource="voice configuration"
          description="The voice configuration couldn't be loaded."
          details={error}
          onRetry={() => VoiceConfig.loadSettings()}
          data-testid="voice-config-error"
        />
      </Card>
    )
  }

  return (
    <Card
      title="Voice configuration"
      data-testid="voice-config-card"
      footer={
        canManage ? (
          <SettingsFormActions
            onSave={form.handleSubmit(handleSave)}
            onCancel={() => form.reset()}
            saving={savingSettings}
            saveTestid="voice-config-save-btn"
            cancelTestid="voice-config-cancel-btn"
          />
        ) : undefined
      }
    >
      {!canManage && (
        <Alert
          data-testid="voice-config-readonly-alert"
          tone="info"
          title="Read-only view"
          description="You can view voice settings but not change them."
          className="mb-3"
        />
      )}

      <Form
        form={form}
        onSubmit={handleSave}
        disabled={!canManage}
        data-testid="voice-config-form"
        layout="horizontal"
      >
        <FormField
          name="enabled"
          label="Enable voice dictation"
          valuePropName="checked"
          description="Master runtime toggle. Users only see the composer mic once a runtime and model are ready."
        >
          <Switch data-testid="voice-config-enabled" />
        </FormField>

        <FormField
          name="model"
          label="Model"
          description="The whisper ggml model used for transcription."
          required
        >
          <Select
            data-testid="voice-config-model"
            className="w-full"
            options={modelOptions}
          />
        </FormField>

        <FormField
          name="model_source_repo"
          label="Model source"
          description="Repo the downloadable model catalog is fetched from (default `ggerganov/whisper.cpp`). Repoint it to an internal mirror or a moved upstream."
          required
        >
          <Input
            data-testid="voice-config-model-source-repo"
            className="w-full"
          />
        </FormField>

        <FormField
          name="language"
          label="Language"
          description="Transcription language. Auto-detect lets whisper infer it per clip."
          required
        >
          <Select
            data-testid="voice-config-language"
            className="w-full"
            options={LANGUAGE_OPTIONS}
          />
        </FormField>

        <Separator titlePlacement="left">
          <Text className="text-xs" type="secondary">
            Live captions
          </Text>
        </Separator>

        <FormField
          name="streaming_enabled"
          label="Enable live captions"
          valuePropName="checked"
          description="Show a live transcript while recording (re-decodes the clip as you speak). Users can still opt out per device; the final transcript is unchanged."
        >
          <Switch data-testid="voice-config-streaming-enabled" />
        </FormField>

        <FormField
          name="stream_interval_ms"
          label="Interim decode interval (ms)"
          description="How often the live caption re-decodes while recording. Raise it on slow hardware or heavy models so interim decodes don't fall behind."
          required
        >
          <InputNumber
            min={300}
            max={10000}
            step={100}
            className="!w-full"
            data-testid="voice-config-stream-interval"
          />
        </FormField>

        <FormField
          name="stream_max_decode_secs"
          label="Max interim decode window (seconds)"
          description="Each live-caption decode is capped to this trailing window, bounding per-tick cost on the shared engine. Clips at/under it are fully captioned; longer ones show recent speech. The final transcript on stop is always complete."
          required
        >
          <InputNumber
            min={5}
            max={600}
            className="!w-full"
            data-testid="voice-config-stream-max-decode"
          />
        </FormField>

        <Separator titlePlacement="left">
          <Text className="text-xs" type="secondary">
            Engine
          </Text>
        </Separator>

        <FormField
          name="idle_unload_secs"
          label="Idle unload timeout (seconds)"
          description="Unload the whisper server after this idle time. 0 disables idle eviction."
          required
        >
          <InputNumber
            min={0}
            max={86400}
            className="!w-full"
            data-testid="voice-config-idle-unload"
          />
        </FormField>

        <FormField
          name="auto_start_timeout_secs"
          label="Auto-start timeout (seconds)"
          description="How long to wait for a freshly-spawned whisper server to become healthy."
          required
        >
          <InputNumber
            min={1}
            max={600}
            className="!w-full"
            data-testid="voice-config-autostart-timeout"
          />
        </FormField>

        <FormField
          name="drain_timeout_secs"
          label="Drain timeout (seconds)"
          description="When unloading, how long to wait for in-flight transcriptions before forcing a stop."
          required
        >
          <InputNumber
            min={1}
            max={600}
            className="!w-full"
            data-testid="voice-config-drain-timeout"
          />
        </FormField>

        <Separator titlePlacement="left">
          <Text className="text-xs" type="secondary">
            Caps
          </Text>
        </Separator>

        <FormField
          name="max_clip_seconds"
          label="Max clip length (seconds)"
          description="Recording auto-stops at this length."
          required
        >
          <InputNumber
            min={1}
            max={600}
            className="!w-full"
            data-testid="voice-config-max-clip"
          />
        </FormField>

        <FormField
          name="max_upload_mib"
          label="Max upload size"
          description="Largest audio clip the transcription endpoint accepts."
          required
        >
          <InputNumber
            min={1}
            max={200}
            suffix="MiB"
            className="!w-full"
            data-testid="voice-config-max-upload"
          />
        </FormField>
      </Form>
    </Card>
  )
}
