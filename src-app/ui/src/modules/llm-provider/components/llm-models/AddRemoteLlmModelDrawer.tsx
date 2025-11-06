import { App, Button, Form } from 'antd'
import { Drawer } from '@/components/common/Drawer'
import { useState } from 'react'
import {
  addLlmModelToProvider,
  loadLlmProviders,
  clearLlmProviderStoreError,
} from '@/modules/llm-provider/store'
import { Stores } from '@/core/stores'
import { ApiClient } from '@/api-client'
import { LlmModelParametersSection } from './shared/LlmModelParametersSection'
import { BASIC_MODEL_FIELDS } from '@/modules/llm-provider/constants/llmModelParameters'

export function AddRemoteLlmModelDrawer() {
  const { message } = App.useApp()
  const [form] = Form.useForm()
  const [loading, setLoading] = useState(false)

  // Get modal state from drawer store
  const { open, providerId } = Stores.AddRemoteLlmModelDrawer

  const handleSubmit = async () => {
    if (!providerId) return

    try {
      setLoading(true)
      clearLlmProviderStoreError()
      const values = await form.validateFields()

      // Create model via API
      // Note: engine_type and file_format are required by the API but only relevant for local models
      const model = await ApiClient.LlmModel.create({
        provider_id: providerId,
        name: values.name,
        display_name: values.display_name,
        description: values.description,
        enabled: true,
        engine_type: 'mistralrs', // Default value for remote models (not used)
        file_format: 'safetensors', // Default value for remote models (not used)
        capabilities: {
          vision: values.vision || false,
          audio: values.audio || false,
          tools: values.tools || false,
          code_interpreter: values.codeInterpreter || false,
          chat: values.chat !== false,
          text_embedding: values.text_embedding || false,
          image_generator: values.image_generator || false,
        },
      })

      // Add to provider in store
      addLlmModelToProvider(providerId, model)
      await loadLlmProviders()

      form.resetFields()
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
    form.resetFields()
    Stores.AddRemoteLlmModelDrawer.closeAddRemoteLlmModelDrawer()
  }

  return (
    <Drawer
      title="Add Remote Model"
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
          Add
        </Button>,
      ]}
      width={600}
      maskClosable={false}
    >
      <Form
        form={form}
        layout="vertical"
        initialValues={{
          enabled: true,
          vision: false,
          audio: false,
          tools: false,
          codeInterpreter: false,
        }}
      >
        <LlmModelParametersSection parameters={BASIC_MODEL_FIELDS} />
      </Form>
    </Drawer>
  )
}
