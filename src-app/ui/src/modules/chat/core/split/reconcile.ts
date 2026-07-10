import { SPLIT_LIMITS } from '@/modules/chat/core/split/limits'
import type { Pane } from '@/modules/chat/core/stores/SplitView.store'

/**
 * The reconciliation reducer (ITEM-25 / DEC-40..43) — the SINGLE pure rule every
 * entry point (sidebar click, ⋯-menu "Open in split pane", drag-drop, the router
 * URL effect) routes a "open conversation X" request through, so the workspace
 * behaves identically no matter how the conversation was opened.
 *
 * v1's open path was `openPane(current) + openPane(null)` — it could only ever
 * produce `[current | new-chat]`, never place two EXISTING conversations side by
 * side (HUMAN_FEEDBACK FB-2/FB-3, "utterly flawed"). This reducer replaces it.
 *
 * **Invariant it preserves (ITEM-24): one conversation per workspace, never a
 * duplicate.** Opening a conversation that is already in a pane FOCUSES that pane
 * regardless of intent — you can't get the same conversation in two panes.
 *
 * The reducer is PURE (`(input) → { next, outcome }`): it computes the next
 * layout and an `outcome` describing what the CALLER must then do in the impure
 * world (navigate the router, show a toast, offer replace-focused). The impure
 * `paneId` generator is injected so the reducer stays deterministic + unit
 * testable (mirrors `SplitView.store.test.ts`).
 */

export type ReconcileIntent = 'auto' | 'newPane' | 'replaceFocused'

/** The slice of `SplitView` state the reducer reads + rewrites. */
export interface WorkspaceLayout {
  panes: Pane[]
  focusedPaneId: string | null
}

export interface ReconcileInput {
  /** Current workspace layout (from the `SplitView` store). */
  layout: WorkspaceLayout
  /**
   * The conversation the URL currently shows (the single-pane base). Used to
   * seed pane 0 when a `newPane` request first opens a split from single-pane
   * mode, so the split is `[current | X]` rather than `[X]`.
   */
  currentConversationId: string | null
  /** The conversation the user is opening. */
  conversationId: string
  projectId?: string | null
  intent: ReconcileIntent
  /** Impure paneId generator, injected for purity/testability. */
  newPaneId: () => string
  /** Override the pane cap (defaults to `SPLIT_LIMITS.MAX_PANES`). */
  maxPanes?: number
}

export type ReconcileOutcome =
  /** No split open → the caller performs a normal router navigate. */
  | { kind: 'navigate'; conversationId: string; projectId: string | null }
  /** The conversation is already open → focus its pane (never duplicate). */
  | { kind: 'focus'; paneId: string; conversationId: string }
  /** A pane was added (split created or extended). */
  | { kind: 'addPane'; paneId: string; conversationId: string }
  /** The focused pane was repointed at the conversation. */
  | { kind: 'replace'; paneId: string; conversationId: string }
  /** `newPane` requested but `MAX_PANES` reached → caller toasts + offers replace. */
  | { kind: 'capReached' }

export interface ReconcileResult {
  next: WorkspaceLayout
  outcome: ReconcileOutcome
}

/**
 * Compute the next workspace layout for opening `conversationId` under `intent`:
 *
 * - **already open** (any intent) → focus that pane, no duplicate.
 * - **`newPane`** → add a pane after the focused one (seeding pane 0 from the URL
 *   conversation when the split is first created); `capReached` at `MAX_PANES`.
 * - **`auto` / `replaceFocused` while split** (≥2 panes) → replace the focused
 *   pane's conversation.
 * - **`auto` / `replaceFocused` while single** (0–1 panes) → normal navigate
 *   (a lone pane is kept in sync so it doesn't dangle on the old conversation).
 */
export function openConversationInWorkspace(
  input: ReconcileInput,
): ReconcileResult {
  const {
    layout,
    currentConversationId,
    conversationId,
    projectId = null,
    intent,
    newPaneId,
    maxPanes = SPLIT_LIMITS.MAX_PANES,
  } = input

  // 1. Already open in a pane → focus it (one-conversation-per-workspace).
  const existing = layout.panes.find(
    (p) => p.conversationId === conversationId,
  )
  if (existing) {
    return {
      next: { panes: layout.panes, focusedPaneId: existing.paneId },
      outcome: { kind: 'focus', paneId: existing.paneId, conversationId },
    }
  }

  // 2. newPane → create or extend the split.
  if (intent === 'newPane') {
    let panes = layout.panes
    let focusedPaneId = layout.focusedPaneId
    // Bootstrapping the split from single-pane mode (empty workspace): seed pane
    // 0 with the conversation the URL currently shows, so the result is
    // `[current | X]` — the whole point of the redesign.
    if (
      panes.length === 0 &&
      currentConversationId &&
      currentConversationId !== conversationId
    ) {
      const base: Pane = {
        paneId: newPaneId(),
        conversationId: currentConversationId,
        projectId: null,
      }
      panes = [base]
      focusedPaneId = base.paneId
    }
    if (panes.length >= maxPanes) {
      return { next: layout, outcome: { kind: 'capReached' } }
    }
    const paneId = newPaneId()
    const pane: Pane = { paneId, conversationId, projectId }
    const idx = focusedPaneId
      ? panes.findIndex((p) => p.paneId === focusedPaneId)
      : -1
    const nextPanes = [...panes]
    if (idx >= 0) nextPanes.splice(idx + 1, 0, pane)
    else nextPanes.push(pane)
    return {
      next: { panes: nextPanes, focusedPaneId: paneId },
      outcome: { kind: 'addPane', paneId, conversationId },
    }
  }

  // 3. auto / replaceFocused while a split is open → replace the focused pane.
  const isSplit = layout.panes.length >= 2
  if (isSplit) {
    const targetId = layout.focusedPaneId ?? layout.panes[0]?.paneId ?? null
    if (targetId) {
      const nextPanes = layout.panes.map((p) =>
        p.paneId === targetId ? { ...p, conversationId, projectId } : p,
      )
      return {
        next: { panes: nextPanes, focusedPaneId: targetId },
        outcome: { kind: 'replace', paneId: targetId, conversationId },
      }
    }
  }

  // 4. Not split (0–1 pane) → normal single-pane navigate. Keep a lone pane in
  //    sync so it doesn't dangle on the previous conversation after the URL moves.
  if (layout.panes.length === 1) {
    const only = layout.panes[0]
    return {
      next: {
        panes: [{ ...only, conversationId, projectId }],
        focusedPaneId: only.paneId,
      },
      outcome: { kind: 'navigate', conversationId, projectId },
    }
  }
  return {
    next: layout,
    outcome: { kind: 'navigate', conversationId, projectId },
  }
}
