import { createModule } from '@/core'
import { Permissions } from '@/api-client/types'
import { History, Plus } from 'lucide-react'
import { AppLayoutDef } from '@/modules/layouts/app-layout'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import { chatBridge } from '@/modules/chat/core/stores/chatBridge'
import { useChatHistoryStore } from '@/modules/chat/stores/ChatHistory.store'
import { useMessageViewStateStore } from '@/modules/chat/core/stores/MessageViewState.store'
import { useSplitViewStore } from '@/modules/chat/core/stores/SplitView.store'
import { RecentConversationsWidget } from '@/modules/chat/widgets/RecentConversationsWidget'
import { OpenInNewWindowAction } from '@/modules/chat/components/OpenInNewWindowAction'
import '@/modules/chat/types'
import '@/modules/chat/core/events' // Import chat events for type merging
import '@/modules/chat/extensions' // Auto-discover and register chat extensions

const NewChatPage = lazyWithPreload(() => import('./pages/NewChatPage'))
const ConversationPage = lazyWithPreload(
  () => import('./pages/ConversationPage'),
)
const ChatHistoryPage = lazyWithPreload(
  () => import('./pages/ChatHistoryPage'),
)

export default createModule({
  metadata: {
    name: 'chat',
    version: '1.0.0',
    description: 'Chat module for conversations',
  },
  dependencies: ['router'],
  stores: [
    {
      // `Stores.Chat` is the focused-pane BRIDGE (forwards to the focused pane,
      // default = the primary pane); single-pane forwards to the primary so
      // behaviour is unchanged.
      name: 'Chat',
      store: chatBridge,
    },
    {
      name: 'ChatHistory',
      store: useChatHistoryStore,
    },
    {
      name: 'MessageViewState',
      store: useMessageViewStateStore,
    },
    {
      name: 'SplitView',
      store: useSplitViewStore,
    },
  ],
  routes: [
    {
      path: '/',
      element: NewChatPage,
      requiresAuth: true,
      layout: AppLayoutDef,
    },
    {
      path: '/chat',
      element: NewChatPage,
      requiresAuth: true,
      layout: AppLayoutDef,
    },
    {
      path: '/chat/:conversationId',
      element: ConversationPage,
      requiresAuth: true,
      layout: AppLayoutDef,
    },
    {
      // Desktop pop-out target (FB-12 / ITEM-52): a native WebviewWindow loads THIS
      // route, which has NO `layout` — so `ConversationPage` renders WITHOUT the app
      // shell (no LeftSidebar / nav), i.e. the chat interface only, not a whole
      // second app. Same `:conversationId` param name so ConversationPage's
      // `useParams()` is unchanged. Web pop-out keeps /chat/:id (the whole app).
      path: '/chat-window/:conversationId',
      element: ConversationPage,
      requiresAuth: true,
    },
    {
      path: '/chats',
      element: ChatHistoryPage,
      requiresAuth: true,
      // Route gate MUST match the `chats` nav slot's ConversationsRead —
      // without it, a user lacking conversations::read had the menu item
      // hidden but could still deep-link /chats and render the full
      // conversation history (no 403). The base chat routes (/, /chat,
      // /chat/:id) stay ungated by design (new-chat is always available).
      permission: Permissions.ConversationsRead,
      layout: AppLayoutDef,
    },
  ],
  slots: {
    sidebarPrimaryActions: [
      {
        id: 'new-chat',
        icon: <Plus />,
        label: 'New Chat',
        to: '/chat',
        order: 10,
      },
    ],
    sidebarNavigation: [
      {
        id: 'chats',
        icon: <History />,
        label: 'Chats',
        path: '/chats',
        order: 10,
        permission: Permissions.ConversationsRead,
      },
    ],
    sidebarContent: [
      {
        id: 'recent-conversations',
        component: RecentConversationsWidget,
        order: 10,
        // Gate: this widget lists the user's conversations and fetches
        // them on mount (`Stores.ChatHistory.loadConversations()`). The
        // sibling `chats` nav entry is gated on ConversationsRead — match
        // it here so a user without the grant never sees the list nor
        // fires the 403 fetch.
        permission: Permissions.ConversationsRead,
      },
    ],
    // Per-conversation header decoration: "Open in new window / tab".
    // order 30 = before the desktop host-mount control (order 40).
    chatConversationHeaderTrailing: [
      {
        id: 'chat-open-in-new-window',
        order: 30,
        component: OpenInNewWindowAction,
      },
    ],
  },
  initialize: () => {
    console.log('Chat module initialized')
  },
})
