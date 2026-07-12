import { useState } from 'react'
import {
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
import { useIsPopoutWindow } from '@/modules/chat/core/popout/useIsPopoutWindow'

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

  // The "back to conversation list" arrow is a WINDOW-WIDE navigate('/chats'), which
  // predates the split + the pop-out and mis-behaves in both (ITEM-55 / FB-13):
  //  - in a SPLIT a per-pane back click collapsed the WHOLE split (panes have their
  //    own ✕ close), and
  //  - in the chat-only pop-out WINDOW it navigated to /chats, pulling the whole app
  //    shell into the window (undoing the chat-only ITEM-52).
  // So show it ONLY in the normal single-pane view: hide when a split is open (panes
  // >= 2, reactive) or in the pop-out window.
  const isSplit = Stores.SplitView.panes.length >= 2
  const isPopoutWindow = useIsPopoutWindow()
  const showBackButton = !isSplit && !isPopoutWindow

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
    // Plain horizontal <form> (NOT the kit <Form>/<FormField>): the kit Form wraps
    // children in a vertical FieldGroup whose responsive Field only becomes a row at
    // an @md container width, which collapsed the input to ~20px and stacked the
    // buttons below it. A flat flex row with the Input registered directly keeps the
    // input growing (flex-1) and the confirm/cancel buttons on the same line.
    return (
      <form
        data-testid="chat-title-editor-form"
        onSubmit={form.handleSubmit(handleSave)}
        className="flex items-center gap-1 flex-1 max-w-full"
      >
        <Input
          {...form.register('title')}
          data-testid="chat-title-input"
          aria-label="Conversation title"
          placeholder="Enter conversation title"
          autoFocus
          size="sm"
          className="flex-1 min-w-0 !border-none !shadow-none bg-transparent text-base font-semibold"
        />
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
          type="button"
          variant="ghost"
          size="default"
          icon={<X />}
          onClick={handleCancel}
          aria-label="Cancel editing title"
          className="!p-1"
        />
      </form>
    )
  }

  return (
    <div className="flex gap-1 items-center justify-start overflow-hidden">
      {showBackButton && (
        <Button
          variant="ghost"
          className="!px-1"
          onClick={handleBack}
          aria-label="Back to conversation list"
          data-testid="conversation-back-button"
        >
          <IoIosArrowBack className="text-md" />
        </Button>
      )}
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
          variant="ghost"
          icon={<Pencil />}
          onClick={handleEditClick}
          aria-label="Edit conversation title"
        />
      </Tooltip>
    </div>
  )
}
