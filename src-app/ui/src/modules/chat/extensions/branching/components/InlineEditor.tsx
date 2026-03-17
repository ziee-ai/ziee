import { App, Button, Input, Space } from 'antd'
import { Stores } from '@/core/stores'

/**
 * Inline editor rendered inside a user message bubble when the user clicks Edit.
 * Replaces the message content area inside ChatMessage while editing is active.
 *
 * On "Save & Submit":
 *   - Creates a new branch from the original message position (via
 *     create_branch_from_message_id in the next sendMessage call).
 *   - Sends the edited text as the new user message on that branch.
 *   - The AI then generates a fresh response.
 *
 * On "Cancel":
 *   - Restores the original message content with no side effects.
 */
export function InlineEditor() {
  const { message } = App.useApp()
  const { editingText } = Stores.Chat.BranchingStore
  const { isStreaming, sending } = Stores.Chat
  const isBusy = isStreaming || sending

  const handleSave = async () => {
    if (!editingText.trim()) {
      message.warning('Message cannot be empty')
      return
    }
    try {
      await Stores.Chat.__state.BranchingStore.confirmEdit()
    } catch (err: any) {
      message.error(err?.message || 'Failed to submit edit')
    }
  }

  const handleCancel = () => {
    Stores.Chat.__state.BranchingStore.cancelEditing()
  }

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if ((e.metaKey || e.ctrlKey) && e.key === 'Enter') {
      e.preventDefault()
      handleSave()
    }
    if (e.key === 'Escape') {
      handleCancel()
    }
  }

  return (
    <div className="w-full flex flex-col gap-2">
      <Input.TextArea
        value={editingText}
        onChange={e =>
          Stores.Chat.__state.BranchingStore.updateEditingText(e.target.value)
        }
        onKeyDown={handleKeyDown}
        autoSize={{ minRows: 1, maxRows: 12 }}
        autoFocus
        disabled={isBusy}
      />

      <Space size={8} className="justify-end flex">
        <Button size="small" onClick={handleCancel} disabled={isBusy}>
          Cancel
        </Button>
        <Button
          size="small"
          type="primary"
          loading={isBusy}
          onClick={handleSave}
        >
          Save & Submit
        </Button>
      </Space>
    </div>
  )
}
