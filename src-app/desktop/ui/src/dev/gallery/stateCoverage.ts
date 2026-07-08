/**
 * STATE-GRANULARITY COVERAGE — the TSC-ENFORCED gate (PART 1), desktop workspace.
 *
 * Mirrors src-app/ui's stateCoverage.ts: `STATE_COVERAGE satisfies
 * Record<RequiredState, StateCoverageEntry>` so a newly-extracted conditional
 * render (the generated "surface:state" union in stateMatrix.generated.ts) with
 * no entry is a COMPILE error, and every gap is excused in code with a reason.
 * See the ui workspace's file for the full rationale.
 */
import type { RequiredState } from './stateMatrix.generated'

export interface StateDelivered {
  via: string
}
export interface AllowlistedGap {
  skip: true
  reason: string
}
export type StateCoverageEntry = StateDelivered | AllowlistedGap

export const STATE_COVERAGE = {
  "modules/host-mount/conversation-extension/components/ConversationMountsControl:empty": { skip: true, reason: "static surface — rendered within its page; 'empty' branch proven by Part 2 runtime coverage" },
  "modules/host-mount/conversation-extension/components/ConversationMountsControl:open": { skip: true, reason: "static surface — rendered within its page; 'open' branch proven by Part 2 runtime coverage" },
  "modules/host-mount/project-extension/components/ProjectMountsPanel:delayed": { skip: true, reason: "via surface — rendered within its page; 'delayed' branch proven by Part 2 runtime coverage" },
  "modules/host-mount/project-extension/components/ProjectMountsPanel:empty": { skip: true, reason: "via surface — rendered within its page; 'empty' branch proven by Part 2 runtime coverage" },
  "modules/layouts/app-layout/components/Drawer:empty": { skip: true, reason: "via surface — rendered within its page; 'empty' branch proven by Part 2 runtime coverage" },
  "modules/remote-access/pages/RemoteAccessPage:delayed": { via: 'page-state-mode' },
  "modules/remote-access/pages/RemoteAccessPage:error": { via: 'page-state-mode' },
  "modules/updater/components/UpdateBanner:error": { skip: true, reason: "via surface — rendered within its page; 'error' branch proven by Part 2 runtime coverage" },
  "modules/updater/pages/AboutPage:error": { via: 'page-state-mode' },
  "modules/office-bridge/chat-extension/extension:panel-open": { skip: true, reason: "via surface — the office-documents panel opens inside the chat right-panel; proven by TEST-18 (desktop/ui e2e)" },
  "modules/office-bridge/components/OpenDocumentsPanel:delayed": { skip: true, reason: "via surface — the loading branch is proven by runtime coverage + TEST-18" },
  "modules/office-bridge/components/OpenDocumentsPanel:empty": { skip: true, reason: "via surface — the empty ('No open Office documents') branch is proven by runtime coverage" },
  "modules/office-bridge/components/OpenDocumentsPanel:error": { skip: true, reason: "via surface — the refetch-failure error branch is proven by runtime coverage" },
  // <<< state-scaffold-insert >>>
} satisfies Record<RequiredState, StateCoverageEntry>

export function stateCoverageSummary() {
  let delivered = 0
  let gaps = 0
  for (const v of Object.values(STATE_COVERAGE) as StateCoverageEntry[]) {
    if ('skip' in v) gaps++
    else delivered++
  }
  return { total: delivered + gaps, delivered, gaps }
}
