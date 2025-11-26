import type { Group } from '@/api-client/types'

/**
 * Props for group widget components.
 */
export interface GroupWidgetProps {
  group: Group
}

/**
 * Interface for widgets that appear in group list items.
 * Each module can register a widget to display resource assignments.
 *
 * Widgets are declared in module metadata and automatically registered
 * by the module system during initialization.
 *
 * @example
 * ```typescript
 * // In widgets/LLMProviderGroupWidget.tsx
 * export function LLMProviderGroupWidget({ group }: GroupWidgetProps) {
 *   return <div>...</div>
 * }
 *
 * // In module.tsx
 * const LLMProviderGroupWidgetComponent = lazyWithPreload(() => import('./widgets/LLMProviderGroupWidget').then(m => ({ default: m.LLMProviderGroupWidget })))
 *
 * export default createModule({
 *   slots: {
 *     userGroup: [
 *       { order: 10, component: LLMProviderGroupWidgetComponent }
 *     ]
 *   }
 * })
 * ```
 */
export interface GroupWidget {
  /**
   * Display order for multiple widgets.
   * Lower numbers appear first.
   * @example 10, 20, 30, ...
   */
  order: number

  /**
   * Component function that renders the widget.
   * Should be a lazy-loaded component created with lazyWithPreload.
   */
  component:
    | React.ComponentType<GroupWidgetProps>
    | (() => Promise<{ default: React.ComponentType<GroupWidgetProps> }>)
}

/**
 * Register the 'userGroup' slot.
 * Other modules can register widgets for this slot to display in group list items.
 */
declare module '@/core/module-system/types' {
  interface Slots {
    userGroup: GroupWidget[]
  }
}
