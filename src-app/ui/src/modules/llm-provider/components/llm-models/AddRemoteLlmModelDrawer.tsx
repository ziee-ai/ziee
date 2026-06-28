import { Button, Form, message, useForm, zodResolver } from '@/components/ui'
import { z } from 'zod'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { useState } from 'react'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'
import { LlmModelParametersSection } from '@/modules/llm-provider/components/llm-models/shared/LlmModelParametersSection'
import { BASIC_MODEL_FIELDS } from '@/modules/llm-provider/constants/llmModelParameters'

const schema = z.object({ name: z.string().min(1, 'Name is required') }).passthrough()

export function AddRemoteLlmModelDrawer() {
  const [loading, setLoading] = useState(false)

  // Get modal state from drawer store
  const { open, providerId } = Stores.AddRemoteLlmModelDrawer
  const canCreate = usePermission(Permissions.LlmModelsCreate)

  const form = useForm<any>({
    resolver: zodResolver(schema),
    defaultValues: {
      enabled: true,
      vision: false,
      audio: false,
      tools: false,
      codeInterpreter: false,
    },
  })

  const onSubmit = async (values: any) => {
    if (!providerId) return

    try {
      setLoading(true)
      Stores.LlmProvider.clearLlmProviderStoreError()

      // Create model via the store (which calls the API + updates
      // local provider state + refreshes the providers list).
      // Note: engine_type and file_format are required by the API
      // but only relevant for local models.
      await Stores.LlmProvider.createLlmModel(providerId, {
        name: values.name,
        display_name: values.display_name,
        description: values.description,
        enabled: true,
        engine_type: 'mistralrs',
        file_format: 'safetensors',
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
            onClick={form.handleSubmit(onSubmit)}
          >
            Add
          </Button>
        ),
      ]}
      size={600}
      mask={{ closable: false }}
    >
      <Form form={form} onSubmit={onSubmit} layout="vertical">
        <LlmModelParametersSection parameters={BASIC_MODEL_FIELDS} />
      </Form>
    </Drawer>
  )
}
