import {
  Alert,
  Button,
  Card,
  Combobox,
  type ComboboxOption,
  Flex,
  Form,
  FormField,
  Input,
  InputNumber,
  Switch,
  message,
  useForm,
} from '@/components/ui'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { useEffect, useMemo, useState } from 'react'
import type { DiscoveredModel } from '@/api-client/types'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'
import { LlmModelParametersSection } from '@/modules/llm-provider/components/llm-models/shared/LlmModelParametersSection'
import { BASIC_MODEL_FIELDS } from '@/modules/llm-provider/constants/llmModelParameters'
import { mapDiscoveredModelToForm } from '@/modules/llm-provider/components/llm-models/discoveredModelForm'

// The picker sources its options from `GET /discover-models` (catalog + live
// /v1/models). display_name + description reuse BASIC_MODEL_FIELDS (minus the
// `name`/Model-ID field, which the picker or the custom-id input owns).
const META_FIELDS = BASIC_MODEL_FIELDS.filter(f => f.name !== 'name')

function optionLabel(m: DiscoveredModel): string {
  const parts = [m.display_name || m.id]
  if (m.context_length) parts.push(`${Math.round(m.context_length / 1000)}k ctx`)
  if (m.deprecated) parts.push('deprecated')
  return parts.join(' · ')
}

export function AddRemoteLlmModelDrawer() {
  const [loading, setLoading] = useState(false)
  const [useCustomId, setUseCustomId] = useState(false)
  const form = useForm<Record<string, unknown>>({
    defaultValues: {
      name: '',
      display_name: '',
      description: '',
      enabled: true,
      vision: false,
      audio: false,
      tools: false,
      codeInterpreter: false,
      chat: true,
      text_embedding: false,
      image_generator: false,
      context_length: undefined,
    },
  })

  const { open, providerId } = Stores.AddRemoteLlmModelDrawer
  const canCreate = usePermission(Permissions.LlmModelsCreate)

  // Discovered models + state for THIS provider. Read the reactive proxy fields
  // UNCONDITIONALLY (each access calls a store hook — a ternary around them would
  // change the hook count when providerId toggles null<->value on open/close and
  // crash with "rendered more/fewer hooks"), then index by providerId plainly.
  const discoveredMap = Stores.LlmProvider.discoveredModels
  const discoverLoadingMap = Stores.LlmProvider.discoverLoading
  const discoverNotesMap = Stores.LlmProvider.discoverNotes
  const discovered = providerId ? discoveredMap[providerId] : undefined
  const discovering = providerId ? Boolean(discoverLoadingMap[providerId]) : false
  const notes = providerId ? discoverNotesMap[providerId] : undefined

  // Fetch the provider's available models when the drawer opens.
  useEffect(() => {
    if (open && providerId) {
      Stores.LlmProvider.discoverModels(providerId)
    }
  }, [open, providerId])

  const options: ComboboxOption[] = useMemo(
    () => (discovered || []).map(m => ({ value: m.id, label: optionLabel(m) })),
    [discovered],
  )

  const selectedName = (form.watch('name') as string) || ''

  // On pick: fill the id + auto-populate display name, capabilities and context
  // from the discovered model. Every field stays user-overridable afterwards.
  const handlePick = (id: string) => {
    form.setValue('name', id, { shouldValidate: true })
    const m = (discovered || []).find(x => x.id === id)
    if (!m) return
    const fields = mapDiscoveredModelToForm(m)
    form.setValue('display_name', fields.display_name)
    form.setValue('vision', fields.vision)
    form.setValue('tools', fields.tools)
    form.setValue('text_embedding', fields.text_embedding)
    form.setValue('chat', fields.chat)
    form.setValue('context_length', fields.context_length ?? undefined)
  }

  const resetAll = () => {
    form.reset()
    setUseCustomId(false)
  }

  const onValid = async (values: Record<string, unknown>) => {
    if (!providerId) return
    const name = (values.name as string)?.trim()
    if (!name) {
      message.error('Select or enter a model ID')
      return
    }

    try {
      setLoading(true)
      Stores.LlmProvider.clearLlmProviderStoreError()

      const ctx = values.context_length
      await Stores.LlmProvider.createLlmModel(providerId, {
        name,
        display_name: (values.display_name as string) || name,
        description: values.description as string,
        enabled: true,
        engine_type: 'mistralrs',
        file_format: 'safetensors',
        capabilities: {
          vision: (values.vision as boolean) || false,
          audio: (values.audio as boolean) || false,
          tools: (values.tools as boolean) || false,
          code_interpreter: (values.codeInterpreter as boolean) || false,
          chat: (values.chat as boolean) !== false,
          text_embedding: (values.text_embedding as boolean) || false,
          image_generator: (values.image_generator as boolean) || false,
          context_length: ctx ? Number(ctx) : undefined,
        },
      })

      resetAll()
      Stores.AddRemoteLlmModelDrawer.closeAddRemoteLlmModelDrawer()
      message.success('Model added successfully')
    } catch (error) {
      console.error('Failed to add model:', error)
      message.error('Failed to create model')
    } finally {
      setLoading(false)
    }
  }

  const handleCancel = () => {
    resetAll()
    Stores.AddRemoteLlmModelDrawer.closeAddRemoteLlmModelDrawer()
  }

  return (
    <Drawer
      title="Add Remote Model"
      open={open}
      onClose={handleCancel}
      footer={[
        <Button key="cancel" variant="outline" onClick={handleCancel} data-testid="llm-add-remote-cancel-btn">
          {canCreate ? 'Cancel' : 'Close'}
        </Button>,
        canCreate && (
          <Button
            key="submit"
            loading={loading}
            onClick={() => form.handleSubmit(onValid)()}
            data-testid="llm-add-remote-submit-btn"
          >
            Add
          </Button>
        ),
      ]}
      size={600}
      mask={{ closable: false }}
      // The shared Drawer body is px-3 pb-4 with NO top padding, so the first
      // Card's top border sits flush against the scroll viewport edge and gets
      // clipped once the content overflows. pt-3 gives it clearance.
      classNames={{ body: 'pt-3' }}
    >
      <Form form={form} onSubmit={onValid} layout="vertical" data-testid="llm-add-remote-model-form">
        <Card title="Model" data-testid="llm-remote-model-card">
          <Flex vertical className="gap-3 w-full">
            {!useCustomId ? (
              <Combobox
                options={options}
                value={selectedName}
                onChange={handlePick}
                loading={discovering}
                placeholder="Select a model"
                searchPlaceholder="Search models…"
                emptyText={discovering ? 'Loading models…' : 'No models found — use a custom ID'}
                aria-label="Model"
                data-testid="llm-remote-model-picker"
              />
            ) : (
              <FormField name="name" aria-label="Model ID" className="mb-0">
                <Input placeholder="e.g., gpt-4o" data-testid="llm-remote-model-custom-id" />
              </FormField>
            )}

            <Flex align="center" justify="between">
              <span className="text-muted-foreground text-sm">Enter a custom model ID</span>
              <Switch
                checked={useCustomId}
                onCheckedChange={v => {
                  setUseCustomId(v)
                  form.setValue('name', '')
                }}
                aria-label="Enter a custom model ID"
                data-testid="llm-remote-custom-id-toggle"
              />
            </Flex>

            {notes && notes.length > 0 && (
              <Alert
                tone="info"
                data-testid="llm-remote-discover-notes"
                title={notes.join(' ')}
              />
            )}
          </Flex>
        </Card>

        <LlmModelParametersSection parameters={META_FIELDS} />

        <Card title="Capabilities" data-testid="llm-remote-capabilities-card">
          <Flex vertical className="gap-1 w-full">
            <CapabilitySwitch label="Chat" name="chat" />
            <CapabilitySwitch label="Vision" name="vision" />
            <CapabilitySwitch label="Tools" name="tools" />
            <CapabilitySwitch label="Audio" name="audio" />
            <CapabilitySwitch label="Code Interpreter" name="codeInterpreter" />
            <CapabilitySwitch label="Text Embedding" name="text_embedding" />
            <CapabilitySwitch label="Image Generator" name="image_generator" />
            <Flex align="center" justify="between" className="gap-3 min-h-9">
              <span className="text-sm">Context window (tokens)</span>
              {/* w-40 (not the Field default w-full) so the field doesn't
                  stretch across the row and shove the label; justify-between
                  then right-aligns it. */}
              <FormField name="context_length" aria-label="Context window" className="mb-0 w-40 shrink-0">
                <InputNumber min={0} placeholder="e.g., 128000" data-testid="llm-remote-context-length" />
              </FormField>
            </Flex>
          </Flex>
        </Card>
      </Form>
    </Drawer>
  )
}

function CapabilitySwitch({ label, name }: { label: string; name: string }) {
  return (
    <Flex align="center" justify="between" className="gap-3 min-h-9">
      <span className="text-sm">{label}</span>
      {/* w-auto shrink-0: the Field defaults to w-full (field.tsx), which would
          stretch across the row and pull the Switch up against the label —
          override it so justify-between can right-align the toggle. */}
      <FormField
        name={name}
        aria-label={label}
        valuePropName="checked"
        className="mb-0 w-auto shrink-0"
      >
        <Switch data-testid={`llm-remote-capability-${name}`} />
      </FormField>
    </Flex>
  )
}
