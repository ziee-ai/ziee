import type { InstallTaskState } from '@/api-client/types'

/**
 * Reconcile the install POST's initial `202` task with whatever the SSE stream
 * may already have tracked for this `(version, arch, flavor, package)` key.
 *
 * The `installVersion` POST and the live-progress SSE race: the server emits
 * `taskStarted` (and often the first `progress` events, e.g. `downloading`)
 * before — or while — the POST's `await` resolves. Once the SSE has created the
 * task for this key it is **authoritative**: it may have already advanced the
 * phase, so a late POST reply (which carries the initial `phase: null`) must NOT
 * overwrite it. Otherwise a long download (the ~1.6 GB `full` flavor) would
 * appear stuck on "queued" until the next discrete phase event, because there
 * are no byte-level progress events during the fetch itself.
 *
 * Kept as a pure function so the anti-clobber invariant is unit-testable without
 * standing up the whole store + its ApiClient/SSE boundary.
 */
export function reconcileInitialTask(
  existing: InstallTaskState | undefined,
  initial: InstallTaskState,
): InstallTaskState {
  return existing ?? initial
}
