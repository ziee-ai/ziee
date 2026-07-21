import { useEffect, useState } from 'react'
import {
  Button,
  Flex,
  Form,
  FormField,
  Input,
  Textarea,
  message,
  useForm,
  zodResolver,
} from '@ziee/kit'
import { z } from 'zod'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { usePermission } from '@/core/permissions'
import { type KnowledgeBase } from '@/api-client/types'
import { Permissions } from '@/api-client/permissions'
import { KnowledgeBases } from '@/modules/knowledge-base/stores/knowledgeBases'

interface Values {
  name: string
  description?: string
}

const schema = z.object({
  name: z.string().min(1, 'Name is required').max(255, 'Name is too long'),
  description: z.string().max(4096, 'Description is too long').optional(),
})

interface Props {
  open: boolean
  editing: KnowledgeBase | null
  onClose: () => void
}

export function KnowledgeBaseFormDrawer({ open, editing, onClose }: Props) {
  const canSave = usePermission(Permissions.KnowledgeBaseManage)
  const isEdit = !!editing
  const [saving, setSaving] = useState(false)

  const form = useForm<Values>({
    resolver: zodResolver(schema),
    defaultValues: { name: '', description: '' },
  })

  useEffect(() => {
    if (open) {
      form.reset({
        name: editing?.name ?? '',
        description: editing?.description ?? '',
      })
    }
  }, [open, editing, form])

  const handleClose = () => {
    if (saving) return
    onClose()
  }

  const handleSubmit = async (values: Values) => {
    setSaving(true)
    try {
      if (isEdit && editing) {
        await KnowledgeBases.update(editing.id, {
          name: values.name,
          description: values.description ?? '',
        })
        message.success('Knowledge base updated')
      } else {
        await KnowledgeBases.create({
          name: values.name,
          description: values.description,
        })
        message.success('Knowledge base created')
      }
      onClose()
    } catch (err) {
      message.error(err instanceof Error ? err.message : 'Failed to save')
    } finally {
      setSaving(false)
    }
  }

  return (
    <Drawer
      title={isEdit ? 'Edit knowledge base' : 'New knowledge base'}
      open={open}
      onClose={handleClose}
      size={600}
      destroyOnHidden
      data-testid="kb-form-drawer"
      footer={
        <Flex className="justify-end gap-2">
          <Button
            data-testid="kb-form-cancel-button"
            variant="outline"
            onClick={handleClose}
            disabled={saving}
          >
            {canSave ? 'Cancel' : 'Close'}
          </Button>
          {canSave && (
            <Button
              data-testid="kb-form-submit-button"
              type="submit"
              onClick={form.handleSubmit(handleSubmit)}
              loading={saving}
            >
              {isEdit ? 'Save' : 'Create'}
            </Button>
          )}
        </Flex>
      }
    >
      <Form
        data-testid="kb-form"
        form={form}
        layout="vertical"
        disabled={!canSave}
        onSubmit={handleSubmit}
      >
        <FormField name="name" label="Name" required>
          <Input
            data-testid="kb-form-name-input"
            placeholder="e.g. Lab protocols"
            autoFocus
          />
        </FormField>
        <FormField
          name="description"
          label="Description"
          description="For your reference — shown on the knowledge-base card. Not sent to the model."
        >
          <Textarea
            data-testid="kb-form-description-textarea"
            rows={3}
            placeholder="Optional short description"
            maxLength={4096}
          />
        </FormField>
      </Form>
    </Drawer>
  )
}
