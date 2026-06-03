import type React from 'react'

/**
 * Available slots that project-extensions can contribute to.
 *
 * Today there's one slot — `knowledge_kinds` — that hosts each knowledge
 * kind's inline preview (on the project detail page) and management panel
 * (inside the knowledge drawer). Future slots (e.g. project-advanced
 * settings extensions) can be added here as the project page grows.
 *
 * Mirrors `CHAT_SLOTS` in `modules/chat/core/extensions/types.ts`.
 */
export const PROJECT_SLOTS = {
  /**
   * Knowledge contributions for a project — one entry per knowledge
   * kind (`files`, future `urls`, `notes`, etc.). Each contribution
   * supplies BOTH an `inlinePreview` component (rendered in the
   * knowledge card on the project detail page) and a `managePanel`
   * component (rendered inside the knowledge drawer). The same slot
   * is rendered from two surfaces via `<ProjectExtensionSlot
   * view="inlinePreview" | "managePanel" />`.
   */
  knowledge_kinds: {
    description: 'Knowledge contributions (file lists, URL lists, etc.) for a project',
  },
} as const

export type ProjectSlotName = keyof typeof PROJECT_SLOTS

/**
 * Which sub-component of a knowledge_kinds contribution to render.
 */
export type KnowledgeView = 'inlinePreview' | 'managePanel'

/**
 * Contribution for `knowledge_kinds`. Identity is the contributing
 * extension's `name` (set at `register({ name, slots })`); no separate
 * `kind` field.
 */
export interface KnowledgeKindContribution {
  /** Section header text (e.g. "Knowledge files"). */
  label: string
  /** Section header icon. */
  icon?: React.ReactNode
  /** Renders in the project detail page's knowledge card — view-only,
   *  compact. Zero props; reads from `Stores.ProjectDetail.project`. */
  inlinePreview: React.ComponentType
  /** Renders inside the knowledge drawer — full management UX
   *  (upload/select/detach). Zero props; reads from
   *  `Stores.ProjectDetail.project`. */
  managePanel: React.ComponentType
  /** Render order across kinds (lower first). Default 100. */
  order?: number
}

/**
 * Slot-name → contribution-type mapping. The registry uses this to
 * accept only the right shape per slot.
 */
export interface ProjectSlotContributions {
  knowledge_kinds: KnowledgeKindContribution
}

/**
 * Argument shape for `projectExtensionRegistry.register({...})`.
 * Mirrors the chat-extension pattern.
 */
export interface ProjectExtensionRegistration {
  /** Unique extension identity — also used for deduplication. */
  name: string
  /** Partial map: only the slots this extension contributes to. */
  slots: Partial<{
    [K in ProjectSlotName]: ProjectSlotContributions[K]
  }>
}
