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
  "modules/remote-access/pages/RemoteAccessPage:delayed": { via: 'page-state-mode' },
  "modules/remote-access/pages/RemoteAccessPage:error": { via: 'page-state-mode' },
  "modules/updater/components/UpdateBanner:error": { skip: true, reason: "via surface — rendered within its page; 'error' branch proven by Part 2 runtime coverage" },
  "modules/updater/pages/AboutPage:error": { via: 'page-state-mode' },
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
