import { Button, Form, useForm, message } from '@/components/ui'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { useState } from 'react'
import {} from '@/modules/llm-provider/stores'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'
import { LlmModelParametersSection } from '@/modules/llm-provider/components/llm-models/shared/LlmModelParametersSection'
import { BASIC_MODEL_FIELDS } from '@/modules/llm-provider/constants/llmModelParameters'

export function AddRemoteLlmModelDrawer() {
  const [loading, setLoading] = useState(false)
  const form = useForm<Record<string, unknown>>({
    defaultValues: {
      enabled: true,
      vision: false,
      audio: false,
      tools: false,
      codeInterpreter: false,
    },
  })

  // Get modal state from drawer store
  const { open, providerId } = Stores.AddRemoteLlmModelDrawer
  const canCreate = usePermission(Permissions.LlmModelsCreate)

  const onValid = async (values: Record<string, unknown>) => {
    if (!providerId) return

    try {
      setLoading(true)
      Stores.LlmProvider.clearLlmProviderStoreError()

      // Create model via the store (which calls the API + updates
      // local provider state + refreshes the providers list).
      // Note: engine_type and file_format are required by the API
      // but only relevant for local models.
      await Stores.LlmProvider.createLlmModel(providerId, {
        name: values.name as string,
        display_name: values.display_name as string,
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
        },
      })

      form.reset()
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
    form.reset()
    Stores.AddRemoteLlmModelDrawer.closeAddRemoteLlmModelDrawer()
  }

  return (
    <Drawer
      title="Add Remote Model"
      open={open}
      onClose={handleCancel}
      footer={[
        <Button key="cancel" variant="outline" onClick={handleCancel}>
          {canCreate ? 'Cancel' : 'Close'}
        </Button>,
        canCreate && (
          <Button
            key="submit"
            loading={loading}
            onClick={() => form.handleSubmit(onValid)()}
          >
            Add
          </Button>
        ),
      ]}
      size={600}
      mask={{ closable: false }}
    >
      <Form
        form={form}
        onSubmit={onValid}
        layout="vertical"
      >
        <LlmModelParametersSection parameters={BASIC_MODEL_FIELDS} />
      </Form>
    </Drawer>
  )
}
