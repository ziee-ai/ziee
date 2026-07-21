import type { ProviderGroupAssignmentCardGet, ProviderGroupAssignmentCardSet } from '../state'

export default (set: ProviderGroupAssignmentCardSet, _get: ProviderGroupAssignmentCardGet) =>
  (): void => {
    set(s => {
      s.providerGroups.clear()
    })
  }
