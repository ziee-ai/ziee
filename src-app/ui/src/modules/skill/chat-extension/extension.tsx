import {
  type ChatExtension,
  createExtension,
} from '@/modules/chat/core/extensions'
import { SkillMenuItem, SkillConversationDrawerHost } from './SkillMenuItem'

/**
 * Skill chat bridge. Auto-discovered by chat's extension glob over
 * `modules/<name>/chat-extension/extension.tsx`. Purely a UI hook: it
 * adds the per-conversation skills opt-out entry to the composer's "+"
 * dropdown (alongside MCP tools), the same place the MCP per-conversation
 * toggle lives. No SSE handlers, no request mutation — skills are
 * delivered server-side via skill_mcp; this surface only lets the user
 * hide specific skills in the current conversation (Path B).
 */
const skillExtension: ChatExtension = createExtension({
  name: 'skill',
  description: 'Per-conversation skills opt-out entry in the chat composer',
  priority: 55,
  slots: {
    toolbar_plus_items: { component: SkillMenuItem, order: 25 },
    // The Dialog host lives in an always-mounted composer slot (NOT the "+"
    // dropdown item) so it survives the dropdown closing on click.
    input_area_suffix: { component: SkillConversationDrawerHost, order: 25 },
  },
})

export default skillExtension
