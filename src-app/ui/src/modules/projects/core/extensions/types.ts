import type React from 'react'
import type { PermissionExpr } from '@/core/permissions'

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
  /**
   * Configuration panels stacked inside the project detail page's
   * Advanced settings area. Each contribution renders as a self-
   * contained card. MCP defaults use this; future per-project
   * rate limits, retention policies, etc. would too.
   */
  advanced_settings: {
    description: 'Project advanced-settings panels (MCP defaults, rate limits, etc.)',
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
   *  compact. Zero props; reads from `ProjectDetail.project`. */
  inlinePreview: React.ComponentType
  /** Renders inside the knowledge drawer — full management UX
   *  (upload/select/detach). Zero props; reads from
   *  `ProjectDetail.project`. */
  managePanel: React.ComponentType
  /** Render order across kinds (lower first). Default 100. */
  order?: number
  /**
   * Permission gate for this contribution. When set, the whole section
   * (header + inline preview + manage panel) is hidden from a user who
   * lacks the expression — the host filters it out reactively (mirrors
   * the sidebar/settings slot `permission` field). Omit for kinds whose
   * data is already covered by the project page's own gate
   * (`projects::read`); set it for kinds backed by a SEPARATE permission
   * (e.g. citations `citations::use`) so a user without that grant never
   * sees the section (whose backend endpoints would 403). */
  permission?: PermissionExpr
}

/**
 * Contribution for `advanced_settings`. The panel is fully self-
 * contained — it renders its own card/title; the host just stacks
 * panels in order. Zero props; reads from `ProjectDetail.project`
 * + its own extension-specific store.
 */
export interface AdvancedSettingsContribution {
  /** Display name (for logs/debug; the panel renders its own header). */
  label: string
  /** Optional icon (currently unused by the host; panels render their own). */
  icon?: React.ReactNode
  /** The panel component. */
  panel: React.ComponentType
  /** Render order (lower first). Default 100. */
  order?: number
  /** Permission gate for this panel — same semantics as
   *  `KnowledgeKindContribution.permission`. */
  permission?: PermissionExpr
}

/**
 * Slot-name → contribution-type mapping. The registry uses this to
 * accept only the right shape per slot.
 */
export interface ProjectSlotContributions {
  knowledge_kinds: KnowledgeKindContribution
  advanced_settings: AdvancedSettingsContribution
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
