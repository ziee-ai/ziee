import { useState } from 'react'
import {
  Form,
  FormField,
  Input,
  Button,
  Tooltip,
  Title,
  useForm,
  zodResolver,
} from '@/components/ui'
import { z } from 'zod'
import { EditOutlined, CheckOutlined, CloseOutlined } from '@ant-design/icons'
import { IoIosArrowBack } from 'react-icons/io'
import { useNavigate } from 'react-router-dom'
import { Stores } from '@/core/stores'
import { chatExtensionRegistry } from '@/modules/chat/core/extensions'

interface TitleFormValues {
  title: string
}

const schema = z.object({
  title: z
    .string()
    .min(1, 'Please enter a title')
    .max(100, 'Title must be less than 100 characters'),
})

/**
 * TitleEditor Component
 * Self-contained component that accesses conversation from store and handles its own state/events
 */
export function TitleEditor() {
  const form = useForm<TitleFormValues>({
    resolver: zodResolver(schema),
    defaultValues: { title: '' },
  })
  const [isEditing, setIsEditing] = useState(false)
  const navigate = useNavigate()

  // Get conversation from store
  const { conversation } = Stores.Chat

  const handleEditClick = () => {
    form.setValue('title', conversation?.title || '')
    setIsEditing(true)
  }

  const handleSave = async (values: TitleFormValues) => {
    try {
      if (conversation && values.title.trim()) {
        await Stores.Chat.updateConversation({ title: values.title.trim() })
        setIsEditing(false)
      }
    } catch (error) {
      console.error('Failed to update title:', error)
    }
  }

  const handleCancel = () => {
    form.reset()
    setIsEditing(false)
  }

  const handleBack = () => {
    // Per-conversation back-target resolution: chat extensions can
    // override chat's default `/chats` via the `conversationBackHref`
    // hook. First non-undefined wins.
    const backHref = conversation
      ? chatExtensionRegistry.conversationBackHref(conversation)
      : undefined
    navigate(backHref ?? '/chats')
  }

  if (isEditing) {
    return (
      <Form
        name="title-editor"
        form={form}
        onSubmit={handleSave}
        className="flex items-center gap-1 flex-1 max-w-full"
      >
        <FormField name="title" className="!mb-0 flex-1">
          <Input
            placeholder="Enter conversation title"
            autoFocus
            size="sm"
            className="!border-none !shadow-none bg-transparent text-base font-semibold"
          />
        </FormField>
        <Button
          type="submit"
          variant="ghost"
          size="sm"
          icon={<CheckOutlined />}
          className="!p-1"
        />
        <Button
          variant="ghost"
          size="sm"
          icon={<CloseOutlined />}
          onClick={handleCancel}
          className="!p-1"
        />
      </Form>
    )
  }

  return (
    <div className="flex gap-1 items-center justify-start overflow-hidden">
      <Button
        variant="ghost"
        className="!px-1"
        onClick={handleBack}
        aria-label="Back to conversation list"
        data-testid="conversation-back-button"
      >
        <IoIosArrowBack className="text-md" />
      </Button>
      <Title
        level={5}
        className="!m-0 !leading-tight truncate"
      >
        {conversation?.title || 'Untitled Conversation'}
      </Title>
      <Tooltip title="Edit title">
        <Button
          variant="ghost"
          icon={<EditOutlined />}
          onClick={handleEditClick}
          aria-label="Edit conversation title"
        />
      </Tooltip>
    </div>
  )
}
