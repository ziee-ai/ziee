import { Button, Card, Flex, Form, useForm, message } from '@ziee/kit'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { useEffect, useState } from 'react'
import { Stores } from '@/modules/llm-provider/stores'
import { LlmModelCapabilitiesSection } from '@/modules/llm-provider/components/llm-models/shared/LlmModelCapabilitiesSection'
import { LlmModelParametersSection } from '@/modules/llm-provider/components/llm-models/shared/LlmModelParametersSection'
import { LlmModelLlamaCppSettingsSection } from '@/modules/llm-provider/components/llm-models/shared/LlmModelLlamaCppSettingsSection'
import { LlmModelMistralRsSettingsSection } from '@/modules/llm-provider/components/llm-models/shared/LlmModelMistralRsSettingsSection'
import {
  BASIC_MODEL_FIELDS,
  MODEL_PARAMETERS,
} from '@/modules/llm-provider/constants/llmModelParameters'
import type { ModelCapabilities, ModelParameters, UpdateLlmModelRequest } from '@/api-client/types'
import { LlmProvider } from '@/modules/llm-provider/stores/llmProvider'

/**
 * Edit drawer for both local and remote LLM models
 * For local models, additional engine/device settings would be shown (currently stubbed)
 */
export function EditLlmModelDrawer() {
  const [loading, setLoading] = useState(false)
  const form = useForm<Record<string, unknown>>({
    defaultValues: {
      name: '',
      display_name: '',
      description: '',
      capabilities: {},
      parameters: {},
      engine_settings: {},
    },
  })

  const { open, modelId } = Stores.EditLlmModelDrawer
  const currentModel = modelId
    ? LlmProvider.providers
        .flatMap(p => p.llm_models || [])
        .find(m => m.id === modelId)
    : null

  // Find provider that owns this model
  const currentProvider = LlmProvider.providers.find(p =>
    p.llm_models?.some(m => m.id === modelId),
  )

  const isLocalModel = currentProvider?.provider_type === 'local'
  const engineType = currentModel?.engine_type

  useEffect(() => {
    if (currentModel && open) {
      form.reset({
        name: currentModel.name,
        display_name: currentModel.display_name,
        description: currentModel.description,
        capabilities: currentModel.capabilities || {},
        parameters: currentModel.parameters || {},
        engine_settings: currentModel.engine_settings || {},
      })
    }
  }, [currentModel, open])

  const onValid = async (values: Record<string, unknown>) => {
    if (!currentModel || !currentProvider) return

    try {
      setLoading(true)
      // Update via the store (which calls the API + reconciles
      // local provider state).
      await LlmProvider.updateLlmModel(currentModel.id, {
        name: values.name as string,
        display_name: values.display_name as string,
        description: values.description as string,
        capabilities: values.capabilities as ModelCapabilities,
        parameters: values.parameters as ModelParameters,
        // Engine settings only apply to local models.
        ...(isLocalModel
          ? { engine_settings: values.engine_settings as UpdateLlmModelRequest['engine_settings'] }
          : {}),
      })

      Stores.EditLlmModelDrawer.closeEditLlmModelDrawer()
      message.success('Model updated successfully')
    } catch (error) {
      console.error('Failed to update model:', error)
      message.error('Failed to update model')
    } finally {
      setLoading(false)
    }
  }

  const handleCancel = () => {
    form.reset()
    Stores.EditLlmModelDrawer.closeEditLlmModelDrawer()
  }

  return (
    <Drawer
      title={isLocalModel ? 'Edit Local Model' : 'Edit Remote Model'}
      open={open}
      onClose={handleCancel}
      footer={[
        <Button key="cancel" variant="outline" onClick={handleCancel} data-testid="llm-edit-model-cancel-btn">
          Cancel
        </Button>,
        <Button
          key="submit"
          loading={loading}
          onClick={() => form.handleSubmit(onValid)()}
          data-testid="llm-edit-model-save-btn"
        >
          Save
        </Button>,
      ]}
      size={600}
      mask={{ closable: false }}
    >
      <Form name="edit-llm-model-form" form={form} onSubmit={onValid} layout="vertical" data-testid="llm-edit-model-form">
        <LlmModelParametersSection parameters={BASIC_MODEL_FIELDS} />

        <Flex className={`flex-col gap-3`}>
          <LlmModelCapabilitiesSection />

          {/* Local-model engine settings — render the section matching
              the model's engine so its `engine_settings` reach the spawn. */}
          {isLocalModel && engineType === 'llamacpp' && (
            <LlmModelLlamaCppSettingsSection />
          )}
          {isLocalModel && engineType === 'mistralrs' && (
            <LlmModelMistralRsSettingsSection />
          )}

          <Card title="Parameters" data-testid="llm-edit-model-parameters-card">
            <LlmModelParametersSection parameters={MODEL_PARAMETERS} />
          </Card>
        </Flex>
      </Form>
    </Drawer>
  )
}
