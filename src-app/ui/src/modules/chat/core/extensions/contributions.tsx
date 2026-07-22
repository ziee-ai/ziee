/**
 * Component-per-extension collectors for the two chat-extension contributions
 * that are HOOKS (`useSendBlocker`, `useConversationMenu`).
 *
 * The old aggregators (`ChatExtensionRegistry.useSendBlockers` /
 * `useConversationMenuContributions`) called ONE hook per registered extension
 * inside a `for` loop, so their hook count varied with the extension set — only
 * Rules-of-Hooks-safe if that set was frozen for the consumer's lifetime (a
 * fragile invariant that broke the moment an extension registered late, e.g.
 * during a lazy module-load wave → React #310).
 *
 * Here each extension's hook runs in its OWN probe component. A changing set is
 * mount/unmount of probes (which React handles), never a hook-count change in a
 * single component — so this is correct at ANY registration time, with no
 * readiness gate. Results are lifted to the caller with a value/signature guard
 * so unstable contribution identities can't cause an update loop.
 */
import {
  useCallback,
  useEffect,
  useState,
  useSyncExternalStore,
  type ReactNode,
} from 'react'
import type { DropdownItem } from '@ziee/kit'
import type { Conversation } from '@/api-client/types'
import { chatExtensionRegistry } from './registry'
import type { ChatExtension } from './types'

/**
 * Reactive list of registered chat extensions — re-renders the caller when an
 * extension registers/unregisters (so it can mount/unmount a probe per
 * extension). The monotonic version is the stable useSyncExternalStore snapshot.
 */
export function useChatExtensionList(): ChatExtension[] {
  useSyncExternalStore(
    chatExtensionRegistry.subscribeToExtensions,
    chatExtensionRegistry.getExtensionsVersion,
    chatExtensionRegistry.getExtensionsVersion,
  )
  return chatExtensionRegistry.getExtensions()
}

// ───────────────────────── send blocked ─────────────────────────

function SendBlockerProbe({
  extension,
  onChange,
}: {
  extension: ChatExtension
  onChange: (name: string, blocking: boolean) => void
}) {
  const blocker = extension.useSendBlocker?.() ?? null
  const blocking = blocker != null
  // Report a BOOLEAN (stable identity) → the guard in `onChange` bails when it's
  // unchanged, so no update loop even though the blocker object identity varies.
  useEffect(() => {
    onChange(extension.name, blocking)
  }, [extension.name, blocking, onChange])
  // Stop blocking when this probe unmounts (extension unregistered).
  useEffect(() => {
    return () => onChange(extension.name, false)
  }, [extension.name, onChange])
  return null
}

/**
 * Whether ANY extension currently blocks send. Returns `[isBlocked, probes]`;
 * the caller MUST render `probes` (each returns null). Replaces
 * `chatExtensionRegistry.useSendBlockers()` — no hooks-in-a-loop.
 */
export function useSendBlocked(): [boolean, ReactNode] {
  const extensions = useChatExtensionList().filter(e => e.useSendBlocker)
  const [blocking, setBlocking] = useState<Set<string>>(new Set())
  const setBlock = useCallback((name: string, isBlocking: boolean) => {
    setBlocking(prev => {
      if (prev.has(name) === isBlocking) return prev // guard → no re-render/loop
      const next = new Set(prev)
      if (isBlocking) next.add(name)
      else next.delete(name)
      return next
    })
  }, [])
  const probes = extensions.map(ext => (
    <SendBlockerProbe key={ext.name} extension={ext} onChange={setBlock} />
  ))
  return [blocking.size > 0, probes]
}

// ─────────────────────── conversation menu ───────────────────────

interface MenuAgg {
  items: DropdownItem[]
  keepMenuOpen: boolean
}

/** Cheap signature of the meaningful menu shape (ignores unstable handler/icon
 *  identities, which don't affect what's rendered). */
function menuSignature(items: DropdownItem[], keepMenuOpen: boolean): string {
  return JSON.stringify([
    keepMenuOpen,
    items.map(i => {
      const it = i as { key?: unknown; label?: unknown; type?: unknown; danger?: unknown }
      return [it.key, typeof it.label === 'string' ? it.label : '', it.type, it.danger]
    }),
  ])
}

function MenuContributionProbe({
  extension,
  conversation,
  onChange,
}: {
  extension: ChatExtension
  conversation: Conversation
  onChange: (name: string, agg: MenuAgg) => void
}) {
  const contrib = extension.useConversationMenu?.(conversation) ?? { items: [] }
  const items = contrib.items ?? []
  const keepMenuOpen = !!contrib.keepMenuOpen
  const sig = menuSignature(items, keepMenuOpen)
  // Report only when the signature changes → stable content never re-fires the
  // effect, so the aggregation never loops.
  useEffect(() => {
    onChange(extension.name, { items, keepMenuOpen })
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [extension.name, sig, onChange])
  useEffect(() => {
    return () => onChange(extension.name, { items: [], keepMenuOpen: false })
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [extension.name, onChange])
  // Overlays are React nodes → render them HERE (no lifting needed).
  return <>{contrib.overlays}</>
}

/**
 * Render-prop collector for per-conversation menu contributions. Renders one
 * probe per extension (each calls its `useConversationMenu` once) and hands the
 * aggregated `{ items, keepMenuOpen }` to `children`. Overlays render inside the
 * probes. Replaces `useConversationMenuContributions()` — no hooks-in-a-loop, so
 * it is correct even if an extension registers after a row has mounted.
 */
export function ConversationMenuContributions({
  conversation,
  children,
}: {
  conversation: Conversation
  children: (agg: MenuAgg) => ReactNode
}): ReactNode {
  const extensions = useChatExtensionList().filter(e => e.useConversationMenu)
  const [byExt, setByExt] = useState<Map<string, MenuAgg>>(new Map())
  const onChange = useCallback((name: string, agg: MenuAgg) => {
    setByExt(prev => {
      const next = new Map(prev)
      next.set(name, agg)
      return next
    })
  }, [])

  // Aggregate in extension order (a plain data reduce — NOT hooks).
  const items: DropdownItem[] = []
  let keepMenuOpen = false
  for (const ext of extensions) {
    const agg = byExt.get(ext.name)
    if (!agg) continue
    items.push(...agg.items)
    if (agg.keepMenuOpen) keepMenuOpen = true
  }

  return (
    <>
      {extensions.map(ext => (
        <MenuContributionProbe
          key={ext.name}
          extension={ext}
          conversation={conversation}
          onChange={onChange}
        />
      ))}
      {children({ items, keepMenuOpen })}
    </>
  )
}
