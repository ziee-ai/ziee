import { useEffect } from 'react'
import {
  Button,
  Form,
  FormField,
  Input,
  Switch,
  Textarea,
  useForm,
  zodResolver,
  message,
  dialog,
} from '@/components/ui'
import { z } from 'zod'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { Stores } from '@/modules/assistant/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'

// Template assistants vs user assistants gate on different permission
// namespaces. `isTemplate` selects which set applies at render time.
const TEMPLATE_PERMS = {
  create: Permissions.AssistantsTemplateCreate,
  edit: Permissions.AssistantsTemplateEdit,
} as const
const USER_PERMS = {
  create: Permissions.AssistantsCreate,
  edit: Permissions.AssistantsEdit,
} as const

// JSON validator for parameters field
const isValidJSON = (value?: string) => {
  if (!value || !value.trim()) {
    return true
  }
  try {
    JSON.parse(value)
    return true
  } catch (_error) {
    return false
  }
}

const schema = z.object({
  name: z
    .string()
    .min(1, 'Please enter a name')
    .max(255, 'Name must be less than 255 characters'),
  description: z
    .string()
    .max(1000, 'Description must be less than 1000 characters')
    .optional(),
  instructions: z.string().optional(),
  parameters: z
    .string()
    .optional()
    .refine(isValidJSON, 'Please enter valid JSON'),
  is_default: z.boolean().optional(),
  enabled: z.boolean().optional(),
})

interface FormValues {
  name: string
  description?: string
  instructions?: string
  parameters?: string // JSON string
  is_default?: boolean
  enabled?: boolean
}

export function AssistantFormDrawer() {
  const form = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: {
      name: '',
      description: '',
      instructions: '',
      parameters: '',
      enabled: true,
      is_default: false,
    },
  })

  // Use drawer store
  const { open, loading, editingAssistant, isTemplate, isCloning } =
    Stores.AssistantDrawer

  const perms = isTemplate ? TEMPLATE_PERMS : USER_PERMS
  const canCreate = usePermission(perms.create)
  const canEdit = usePermission(perms.edit)
  const canSave = editingAssistant && !isCloning ? canEdit : canCreate

  // Initialize form when drawer opens or editing assistant changes
  useEffect(() => {
    if (open) {
      if (editingAssistant) {
        // Stringify parameters for textarea
        const parametersString = editingAssistant.parameters
          ? JSON.stringify(editingAssistant.parameters, null, 2)
          : ''

        form.reset({
          name: editingAssistant.name,
          description: editingAssistant.description,
          instructions: editingAssistant.instructions,
          parameters: parametersString,
          enabled: editingAssistant.enabled,
          is_default: editingAssistant.is_default,
        })
      } else {
        // Creating new assistant with default values
        form.reset({
          name: '',
          description: '',
          instructions: '',
          parameters: '',
          enabled: true,
          is_default: false,
        })
      }
    }
  }, [open, editingAssistant, form])

  const doClose = () => {
    form.reset()
    Stores.AssistantDrawer.closeAssistantDrawer()
  }

  const handleClose = () => {
    // Guard against losing edits: prompt before discarding when the user has
    // touched any field. A pristine form (just opened / freshly reset) closes
    // immediately. Covers both the footer Cancel button and the drawer's X.
    if (form.formState.isDirty) {
      void dialog
        .confirm({
          title: 'Discard unsaved changes?',
          description:
            'You have unsaved changes. Closing now will discard them.',
          okText: 'Discard',
          cancelText: 'Keep editing',
          danger: true,
        })
        .then(ok => {
          if (ok) doClose()
        })
      return
    }
    doClose()
  }

  const handleParametersBlur = () => {
    const value = form.getValues('parameters')
    if (!value || !value.trim()) return

    try {
      const parsed = JSON.parse(value)
      const prettified = JSON.stringify(parsed, null, 2)
      form.setValue('parameters', prettified)
    } catch (_error) {
      // Invalid JSON, leave as is - validation will show error
    }
  }

  const handleSubmit = async (values: FormValues) => {
    // Parse parameters JSON string
    let parameters: any = undefined
    if (values.parameters && values.parameters.trim()) {
      try {
        parameters = JSON.parse(values.parameters)
      } catch (_error) {
        // JSON validation should catch this, but handle it just in case
        message.error('Invalid JSON in parameters field')
        return
      }
    }

    const payload = {
      name: values.name,
      description: values.description,
      instructions: values.instructions,
      parameters,
      is_default: values.is_default,
      enabled: values.enabled,
    }

    Stores.AssistantDrawer.setAssistantDrawerLoading(true)
    try {
      // If cloning or creating new, always create (not update)
      if (editingAssistant && !isCloning) {
        // Update existing assistant
        if (isTemplate) {
          await Stores.TemplateAssistants.updateTemplateAssistant(
            editingAssistant.id,
            payload,
          )
        } else {
          await Stores.UserAssistants.updateUserAssistant(
            editingAssistant.id,
            payload,
          )
        }
        message.success('Assistant updated successfully')
      } else {
        // Create new assistant (including when cloning from template)
        if (isTemplate) {
          await Stores.TemplateAssistants.createTemplateAssistant(payload)
        } else {
          await Stores.UserAssistants.createUserAssistant(payload)
        }
        message.success('Assistant created successfully')
      }
      Stores.AssistantDrawer.closeAssistantDrawer()
    } catch (error) {
      console.error('Failed to save assistant:', error)
      // Error already shown via store error state
    } finally {
      Stores.AssistantDrawer.setAssistantDrawerLoading(false)
    }
  }

  const getTitle = () => {
    if (isCloning) {
      return 'Create from Template'
    }
    if (editingAssistant) {
      return isTemplate ? 'Edit Template Assistant' : 'Edit Assistant'
    }
    return isTemplate ? 'Create Template Assistant' : 'Create Assistant'
  }

  return (
    <Drawer
      title={getTitle()}
      open={open}
      onClose={handleClose}
      size={600}
      mask={{ closable: false }}
      footer={null}
    >
      <Form
        data-testid="assistant-form"
        name="assistant-form"
        form={form}
        layout="vertical"
        onSubmit={handleSubmit}
        disabled={!canSave}
      >
        <FormField name="name" label="Name">
          <Input
            data-testid="assistant-form-name"
            placeholder="Enter assistant name"
            aria-label="Assistant name"
          />
        </FormField>

        <FormField name="description" label="Description">
          <Textarea
            data-testid="assistant-form-description"
            placeholder="Enter a brief description"
            rows={2}
            aria-label="Assistant description"
          />
        </FormField>

        <FormField name="instructions" label="Instructions">
          <Textarea
            data-testid="assistant-form-instructions"
            placeholder="Enter system instructions for the assistant"
            rows={6}
            maxLength={65536}
            aria-label="Assistant instructions"
          />
        </FormField>

        <FormField
          name="parameters"
          label="Parameters"
          description="Model parameters in JSON format (e.g., temperature, max_tokens, top_p)"
        >
          <Textarea
            data-testid="assistant-form-parameters"
            placeholder='{"temperature": 0.7, "max_tokens": 2048, "top_p": 0.9}'
            rows={6}
            aria-label="Model parameters in JSON format"
            onBlur={handleParametersBlur}
          />
        </FormField>

        <FormField
          name="enabled"
          label="Enabled"
          valuePropName="checked"
          description="Whether this assistant is enabled"
        >
          <Switch data-testid="assistant-form-enabled" />
        </FormField>

        <FormField
          name="is_default"
          label="Set as Default"
          valuePropName="checked"
          description={
            isTemplate
              ? 'Set as the default template assistant for all users'
              : 'Set as your default assistant'
          }
        >
          <Switch data-testid="assistant-form-default" />
        </FormField>

        <div className="flex justify-end gap-3 pt-4">
          <Button data-testid="assistant-form-cancel" variant="outline" onClick={handleClose} disabled={loading}>
            {canSave ? 'Cancel' : 'Close'}
          </Button>
          {canSave && (
            <Button data-testid="assistant-form-submit" type="submit" loading={loading}>
              {editingAssistant ? 'Save' : 'Create'}
            </Button>
          )}
        </div>
      </Form>
    </Drawer>
  )
}
