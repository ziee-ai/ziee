// Adapter: forwards to chat's <ChatInput /> inside the projects/
// chat-extension bridge, so projects/pages/ProjectDetailPage doesn't
// import from @/modules/chat directly.
//
// Why an adapter and not direct import: keeps the project↔chat
// boundary clean. The grep gate `grep -rnE "@/modules/chat"
// src-app/ui/src/modules/projects/` returns only inside
// `projects/chat-extension/` after this round.

import { ChatInput } from '@/modules/chat/components/ChatInput'

export function ProjectInlineChatInput() {
  return <ChatInput />
}
