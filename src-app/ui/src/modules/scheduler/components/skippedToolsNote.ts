/**
 * Human note for a run's `skipped_tools` (DEC-17.5): tools the firing skipped
 * because they weren't on the task's unattended allow-list. `skipped_tools` is a
 * loosely-typed JSONB array on `ScheduledTaskRun`; returns null when empty/absent
 * so the caller can render nothing. Pure + exported so it's unit-testable.
 */
export function skippedToolsNote(skipped: unknown): string | null {
  const n = Array.isArray(skipped) ? skipped.length : 0
  if (n <= 0) return null
  return `${n} tool${n === 1 ? '' : 's'} skipped (not permitted unattended)`
}
