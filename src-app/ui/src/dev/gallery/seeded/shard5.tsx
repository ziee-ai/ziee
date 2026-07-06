/**
 * Shard 5 seeded-surface entries (parallel gap grind).
 *
 * OWNED BY SHARD 5 ONLY. Add `SeededSurfaceEntry` objects for your assigned
 * gaps here. Import helpers from './helpers'. Prefix every slug with
 * `seeded-s5-` so slugs never collide across shards. Do NOT edit
 * seededSurfaces.tsx, overlays.tsx, main.tsx, pages.tsx, stories/index.ts,
 * coverage-allowlist.json, or any generated matrix — those are integrator-owned.
 *
 * Shard 5 scope: chat (non-widget), auth, projects, user*, summarization,
 * onboarding. See /data/pbya/ziee/tmp/gapgrind-shards.md for the gap list.
 */
import { lazyNamed, lazyProps, holdForever, holdPatch, whenTrue } from './helpers'
import type { SeededSurfaceEntry } from './helpers'

export const shard5Seeded: SeededSurfaceEntry[] = [
  // ── ConversationPage: still-loading (loading && !conversation). ──────────────
  // The GET-driven pass can't hold a page mid-load; seed loading:true + no
  // conversation so the `<Loading/>` early return (line 101) renders.
  {
    slug: 'seeded-s5-conversation-loading',
    title: 'Conversation page — loading',
    note: 'loading && !conversation → the page load spinner (ConversationPage:101)',
    path: '/chat/:conversationId',
    initialPath: '/chat/s5-loading',
    component: lazyNamed(
      () => import('@/modules/chat/pages/ConversationPage'),
      'default',
    ),
    setup: async () => {
      const { useChatStore } = await import(
        '@/modules/chat/core/stores/Chat.store'
      )
      await holdPatch(() =>
        useChatStore.setState({ loading: true, conversation: null } as any),
      )
    },
  },
  // ── ConversationPage: not-found (!loading && !conversation). ─────────────────
  {
    slug: 'seeded-s5-conversation-not-found',
    title: 'Conversation page — not found',
    note: '!loading && !conversation → "Conversation not found" alert (ConversationPage:108)',
    path: '/chat/:conversationId',
    initialPath: '/chat/s5-missing',
    component: lazyNamed(
      () => import('@/modules/chat/pages/ConversationPage'),
      'default',
    ),
    setup: async () => {
      const { useChatStore } = await import(
        '@/modules/chat/core/stores/Chat.store'
      )
      await holdPatch(() =>
        useChatStore.setState({
          loading: false,
          conversation: null,
          error: null,
        } as any),
      )
    },
  },
  // ── ConversationPage: loaded conversation + a send/stream error banner. ──────
  // Load the real showcase conversation (passes the two !conversation early
  // returns), then seed `error` so the inline error banner (line 142) renders.
  {
    slug: 'seeded-s5-conversation-error',
    title: 'Conversation page — error banner',
    note: 'conversation loaded + Stores.Chat.error → the inline error banner (ConversationPage:142)',
    path: '/chat/:conversationId',
    initialPath: '/chat/11111111-1111-1111-1111-111111111111',
    component: lazyNamed(
      () => import('@/modules/chat/pages/ConversationPage'),
      'default',
    ),
    setup: async () => {
      const { useChatStore } = await import(
        '@/modules/chat/core/stores/Chat.store'
      )
      const { SHOWCASE_CONVERSATION_ID } = await import(
        '../fixtures/chat-deep'
      )
      await useChatStore.getState().loadConversation(SHOWCASE_CONVERSATION_ID)
      await whenTrue(
        () =>
          useChatStore.getState().conversation?.id === SHOWCASE_CONVERSATION_ID,
      )
      // setState merges shallow → conversation is preserved, only error/loading flip.
      await holdPatch(() =>
        useChatStore.setState({
          error: 'Failed to send message. Please try again.',
          loading: false,
        } as any),
      )
    },
  },
  // ── AuthGuard: bootstrap loading (isInitializing || needsSetup === null). ────
  // Seed multi-user mode + Auth.isInitializing + App.needsSetup:null so the
  // fullscreen bootstrap spinner (line 47) renders instead of the auth page.
  {
    slug: 'seeded-s5-auth-initializing',
    title: 'Auth guard — bootstrap loading',
    note: 'multiUser && (isInitializing || needsSetup===null) → fullscreen spinner (AuthGuard:47)',
    path: '/',
    initialPath: '/',
    component: lazyProps(
      () => import('@/modules/auth/AuthGuard'),
      'AuthGuard',
      { children: null },
    ),
    setup: async () => {
      const { Auth } = await import('@/modules/auth/Auth.store')
      const { App } = await import('@/modules/app/App.store')
      const { AppMode } = await import('@/modules/app/AppMode.store')
      await holdPatch(() => {
        AppMode.store.setState({ multiUserMode: true } as any)
        App.store.setState({ needsSetup: null } as any)
        Auth.store.setState({
          isInitializing: true,
          isAuthenticated: false,
        } as any)
      })
    },
  },
  // ── UserProfileWidget: auth still resolving (!user && (isInitializing||isLoading)). ─
  {
    slug: 'seeded-s5-user-profile-loading',
    title: 'User profile widget — loading',
    note: '!user && (isInitializing || isLoading) → the skeleton row (UserProfileWidget:86)',
    path: '/',
    initialPath: '/',
    component: lazyNamed(
      () => import('@/modules/user-profile/UserProfileWidget'),
      'UserProfileWidget',
    ),
    setup: async () => {
      const { Auth } = await import('@/modules/auth/Auth.store')
      // holdForever (not holdPatch): the widget can mount after a fixed hold
      // window ends under the full pass, so assert on a permanent interval.
      holdForever(() =>
        Auth.store.setState({
          user: null,
          isInitializing: true,
          isLoading: false,
        } as any),
      )
    },
  },
  // ── ProjectFormDrawer: drawer open in the saving/loading state. ──────────────
  // Seed the drawer open + loading (covers the loading render branches — Cancel
  // disabled, Submit spinner), then fire Escape so `handleClose` runs while
  // loading is true and hits its `if (loading) return` guard (line 129); the
  // guard returns early so the drawer stays open (no unmount).
  {
    slug: 'seeded-s5-project-form-loading',
    title: 'Project form drawer — saving (loading guard)',
    note: 'open && loading → loading render + handleClose `if (loading) return` (ProjectFormDrawer:129)',
    path: '/',
    initialPath: '/',
    component: lazyNamed(
      () => import('@/modules/projects/components/ProjectFormDrawer'),
      'ProjectFormDrawer',
    ),
    setup: async () => {
      const { ProjectDrawer } = await import(
        '@/modules/projects/stores/ProjectDrawer.store'
      )
      const seed = () =>
        ProjectDrawer.store.setState({
          open: true,
          loading: true,
          editingProject: null,
        } as any)
      seed()
      // Let the Radix drawer mount its dismissable layer before Escape.
      await new Promise(r => setTimeout(r, 600))
      for (let i = 0; i < 3; i++) {
        document.dispatchEvent(
          new KeyboardEvent('keydown', {
            key: 'Escape',
            code: 'Escape',
            bubbles: true,
          }),
        )
        seed()
        await new Promise(r => setTimeout(r, 250))
      }
      await holdPatch(seed, 6, 250)
    },
  },
]
