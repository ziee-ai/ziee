/**
 * DELIBERATE DIVERGENCE from core's ProviderGroupAssignmentCard.
 *
 * Returns null because all providers are automatically assigned to all
 * groups in the desktop app via the AutoAssignProviderHandler event
 * handler (the single admin is implicitly a member of every group, so
 * there's nothing for the user to assign).
 */
export function ProviderGroupAssignmentCard() {
  return null
}
