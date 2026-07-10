import { Card, Flex, FormField, Select, Switch, Alert, useWatch } from '@/components/ui'

export function LlmModelCapabilitiesSection() {
  // Memory-plan §8 polish (gap #12): when text_embedding is ticked,
  // the model is an embedder. Chat-only capabilities (chat, vision,
  // audio, tools, code interpreter) and image_generator don't apply
  // — gray them out and surface a hint instead of letting an admin
  // tick contradictory flags.
  const isEmbedding = useWatch({ name: 'capabilities.text_embedding' })
  const grayed = Boolean(isEmbedding)

  return (
    <Card title="Capabilities" data-testid="llm-capabilities-card">
      <Flex vertical className="gap-2 w-full">
        {grayed && (
          <Alert
            tone="info"
            className="!mb-1"
            data-testid="llm-capabilities-embedder-alert"
            title="This model is flagged as an embedder; chat capabilities are hidden because they don't apply."
          />
        )}

        <CapabilityRow
          label="Text Embedding"
          name="text_embedding"
          help="Generates vectors instead of chat text. Used by the Memory module."
        />

        {!grayed && (
          <>
            <CapabilityRow label="Chat" name="chat" />
            <CapabilityRow label="Vision" name="vision" />
            <CapabilityRow label="Audio" name="audio" />
            <CapabilityRow label="Tools" name="tools" />
            <CapabilityRow label="Code Interpreter" name="codeInterpreter" />
            <CapabilityRow label="Image Generator" name="image_generator" />

            {/* Parameter contract overrides. Left on "Auto" the adapter infers
                from the curated catalog + provider model-family policy; set them
                only when a specific model's requirements differ. */}
            <div className="text-muted-foreground mt-2 mb-0.5" style={{ fontSize: 12 }}>
              Parameter contract (leave on Auto unless a model rejects/needs a param)
            </div>
            <TriStateRow
              label="Supports sampling params"
              name="supports_sampling_params"
              help="temperature / top_p / top_k. Set “No” for models that reject them."
            />
            <TriStateRow
              label="Supports thinking"
              name="supports_thinking"
              help="Enable reasoning/thinking for this model."
            />
            <StyleRow label="Thinking style" name="thinking_style" />
          </>
        )}
      </Flex>
    </Card>
  )
}

/** A tri-state (Auto / Yes / No) bound to an `Option<bool>` capability field. */
function TriStateSelect({
  value,
  onChange,
  testid,
}: {
  value?: boolean
  onChange?: (v: boolean | undefined) => void
  testid: string
}) {
  const str = value === undefined || value === null ? '' : value ? 'true' : 'false'
  return (
    <Select
      value={str}
      onChange={v => onChange?.(v === '' ? undefined : v === 'true')}
      options={[
        { value: '', label: 'Auto' },
        { value: 'true', label: 'Yes' },
        { value: 'false', label: 'No' },
      ]}
      className="w-32"
      data-testid={testid}
    />
  )
}

/** Auto / Adaptive / Budget bound to an `Option<string>` field. */
function ThinkingStyleSelect({
  value,
  onChange,
  testid,
}: {
  value?: string
  onChange?: (v: string | undefined) => void
  testid: string
}) {
  return (
    <Select
      value={value ?? ''}
      onChange={v => onChange?.(v === '' ? undefined : v)}
      options={[
        { value: '', label: 'Auto' },
        { value: 'adaptive', label: 'Adaptive' },
        { value: 'budget', label: 'Budget' },
      ]}
      className="w-32"
      data-testid={testid}
    />
  )
}

function TriStateRow({
  label,
  name,
  help,
}: {
  label: string
  name: string
  help?: string
}) {
  return (
    <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
      <span>
        {label}
        {help && (
          <span className="text-muted-foreground" style={{ fontSize: 12, marginLeft: 8 }}>
            {help}
          </span>
        )}
      </span>
      <FormField name={`capabilities.${name}`} aria-label={label} className="mb-0">
        <TriStateSelect testid={`llm-capability-select-${name}`} />
      </FormField>
    </div>
  )
}

function StyleRow({ label, name }: { label: string; name: string }) {
  return (
    <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
      <span>{label}</span>
      <FormField name={`capabilities.${name}`} aria-label={label} className="mb-0">
        <ThinkingStyleSelect testid={`llm-capability-select-${name}`} />
      </FormField>
    </div>
  )
}

function CapabilityRow({
  label,
  name,
  help,
}: {
  label: string
  name: string
  help?: string
}) {
  return (
    <div
      style={{
        display: 'flex',
        justifyContent: 'space-between',
        alignItems: 'center',
      }}
    >
      <span>
        {label}
        {help && (
          <span className="text-muted-foreground" style={{ fontSize: 12, marginLeft: 8 }}>
            {help}
          </span>
        )}
      </span>
      <FormField
        name={`capabilities.${name}`}
        aria-label={label}
        valuePropName="checked"
        className="mb-0"
      >
        <Switch data-testid={`llm-capability-switch-${name}`} />
      </FormField>
    </div>
  )
}
