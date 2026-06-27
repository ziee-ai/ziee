import { Dialog } from '@/components/ui'
import { Stores } from '@/core/stores'
import { ConversationSkillsPanel } from './ConversationSkillsPanel'
import { SkillDetailDrawer } from './SkillDetailDrawer'

interface SkillConversationDrawerProps {
  conversationId: string
}

/**
 * Modal host for the per-conversation skills opt-out panel, opened from
 * the chat composer's "+" dropdown (mirrors McpConfigModal's role for
 * MCP). Mounts a SkillDetailDrawer too so panel rows can open the detail
 * view WITH the conversation id threaded through — that's what makes the
 * drawer's "Hide in this conversation" checkbox reachable from chat.
 */
export function SkillConversationDrawer({
  conversationId,
}: SkillConversationDrawerProps) {
  const { open } = Stores.SkillConversationDrawer

  return (
    <>
      <Dialog
        open={open}
        onOpenChange={(v) => {
          if (!v) Stores.SkillConversationDrawer.closeDrawer()
        }}
        title="Skills in this conversation"
        footer={null}
        className="!max-w-[520px]"
      >
        <ConversationSkillsPanel conversationId={conversationId} />
      </Dialog>
      <SkillDetailDrawer />
    </>
  )
}
