import type { ProviderGroupAssignmentCardGet, ProviderGroupAssignmentCardSet, ProviderGroups } from '../state'

export default (_set: ProviderGroupAssignmentCardSet, get: ProviderGroupAssignmentCardGet) =>
  (providerId: string): ProviderGroups | undefined =>
    get().providerGroups.get(providerId)
