import { Dialog } from '@ziee/kit'
import { ConversationSkillsPanel } from './ConversationSkillsPanel'
import { SkillDetailDrawer } from './SkillDetailDrawer'
import { SkillConversationDrawer as SkillConversationDrawerStore } from '@/modules/skill/stores/skillConversationDrawer'

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
  // Open only when THIS conversation is the one whose drawer was opened — so in a
  // split, clicking "Skills in this chat" in one pane doesn't render every pane's
  // dialog. The detail sub-drawer (a global singleton) mounts only for the open
  // pane, avoiding N stacked copies across panes.
  const open = SkillConversationDrawerStore.openConversationId === conversationId

  return (
    <>
      <Dialog
        open={open}
        data-testid="skill-conversation-dialog"
        onOpenChange={(v) => {
          if (!v) SkillConversationDrawerStore.closeDrawer()
        }}
        title="Skills in this conversation"
        footer={null}
        className="!max-w-[520px]"
      >
        <ConversationSkillsPanel conversationId={conversationId} />
      </Dialog>
      {open && <SkillDetailDrawer />}
    </>
  )
}
