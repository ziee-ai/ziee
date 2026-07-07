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
import { Pencil, Check, X } from 'lucide-react'
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
        data-testid="chat-title-editor-form"
        name="title-editor"
        form={form}
        onSubmit={handleSave}
        className="flex items-center gap-1 flex-1 max-w-full"
      >
        <FormField
          name="title"
          aria-label="Conversation title"
          className="flex-1"
        >
          <Input
            data-testid="chat-title-input"
            aria-label="Conversation title"
            placeholder="Enter conversation title"
            autoFocus
            size="sm"
            className="!border-none !shadow-none bg-transparent text-base font-semibold"
          />
        </FormField>
        <Button
          data-testid="chat-title-save-btn"
          type="submit"
          variant="ghost"
          size="default"
          icon={<Check />}
          aria-label="Save title"
          className="!p-1"
        />
        <Button
          data-testid="chat-title-cancel-btn"
          variant="ghost"
          size="default"
          icon={<X />}
          onClick={handleCancel}
          aria-label="Cancel editing title"
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
        data-testid="conversation-title"
      >
        {conversation?.title || 'Untitled Conversation'}
      </Title>
      <Tooltip title="Edit title">
        <Button
          data-testid="chat-title-edit-btn"
          variant="outline"
          icon={<Pencil />}
          onClick={handleEditClick}
          aria-label="Edit conversation title"
        />
      </Tooltip>
    </div>
  )
}
