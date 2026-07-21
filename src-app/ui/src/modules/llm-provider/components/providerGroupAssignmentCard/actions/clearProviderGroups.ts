import type { ProviderGroupAssignmentCardGet, ProviderGroupAssignmentCardSet } from '../state'

export default (set: ProviderGroupAssignmentCardSet, _get: ProviderGroupAssignmentCardGet) =>
  (providerId: string): void => {
    set(s => {
      s.providerGroups.delete(providerId)
    })
  }
