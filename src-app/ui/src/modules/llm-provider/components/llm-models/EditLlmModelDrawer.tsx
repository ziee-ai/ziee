import { App, Button, Card, Flex, Form } from 'antd'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { useEffect, useState } from 'react'
import { Stores } from '@/modules/llm-provider/stores'
import { LlmModelCapabilitiesSection } from '@/modules/llm-provider/components/llm-models/shared/LlmModelCapabilitiesSection'
import { LlmModelParametersSection } from '@/modules/llm-provider/components/llm-models/shared/LlmModelParametersSection'
import {
  BASIC_MODEL_FIELDS,
  MODEL_PARAMETERS,
} from '@/modules/llm-provider/constants/llmModelParameters'

/**
 * Edit drawer for both local and remote LLM models
 * For local models, additional engine/device settings would be shown (currently stubbed)
 */
export function EditLlmModelDrawer() {
  const { message } = App.useApp()
  const [form] = Form.useForm()
  const [loading, setLoading] = useState(false)

  const { open, modelId } = Stores.EditLlmModelDrawer
  const currentModel = modelId
    ? Stores.LlmProvider.findLlmModelById(modelId)
    : null

  // Find provider that owns this model
  const currentProvider = Stores.LlmProvider.providers.find(p =>
    p.llm_models?.some(m => m.id === modelId),
  )

  const isLocalModel = currentProvider?.provider_type === 'local'

  useEffect(() => {
    if (currentModel && open) {
      form.setFieldsValue({
        name: currentModel.name,
        display_name: currentModel.display_name,
        description: currentModel.description,
        capabilities: currentModel.capabilities || {},
        parameters: currentModel.parameters || {},
      })
    }
  }, [currentModel, open, form])

  const handleSubmit = async () => {
    if (!currentModel || !currentProvider) return

    try {
      setLoading(true)
      const values = await form.validateFields()

      // Update via the store (which calls the API + reconciles
      // local provider state).
      await Stores.LlmProvider.updateLlmModel(currentModel.id, {
        name: values.name,
        display_name: values.display_name,
        description: values.description,
        capabilities: values.capabilities,
        parameters: values.parameters,
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
    form.resetFields()
    Stores.EditLlmModelDrawer.closeEditLlmModelDrawer()
  }

  return (
    <Drawer
      title={isLocalModel ? 'Edit Local Model' : 'Edit Remote Model'}
      open={open}
      onClose={handleCancel}
      footer={[
        <Button key="cancel" onClick={handleCancel}>
          Cancel
        </Button>,
        <Button
          key="submit"
          type="primary"
          loading={loading}
          onClick={handleSubmit}
        >
          Save
        </Button>,
      ]}
      size={600}
      mask={{ closable: false }}
    >
      <Form name="edit-llm-model-form" form={form} layout="vertical">
        <LlmModelParametersSection parameters={BASIC_MODEL_FIELDS} />

        <Flex className={`flex-col gap-3`}>
          <LlmModelCapabilitiesSection />

          {/* TODO: Add engine/device settings for local models once backend supports it */}
          {/* {isLocalModel && (
            <>
              <LlmModelEngineSelectionSection />
              <LlmModelDeviceSelectionSection />
              <LlmModelMistralRsSettingsSection />
              <LlmModelLlamaCppSettingsSection />
            </>
          )} */}

          <Card title="Parameters">
            <LlmModelParametersSection parameters={MODEL_PARAMETERS} />
          </Card>
        </Flex>
      </Form>
    </Drawer>
  )
}
