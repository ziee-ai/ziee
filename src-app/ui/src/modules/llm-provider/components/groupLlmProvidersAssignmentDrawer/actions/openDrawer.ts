import { setOverlayOpen } from '@/core/overlays/overlayVisibility'
import type { Group } from '@/api-client/types'
import type { GroupLlmProvidersAssignmentDrawerGet, GroupLlmProvidersAssignmentDrawerSet } from '../state'

export default (set: GroupLlmProvidersAssignmentDrawerSet, _get: GroupLlmProvidersAssignmentDrawerGet) => {
  return async (group: Group) => {
    set({ isOpen: true, selectedGroup: group })
    setOverlayOpen('group-llm-assignment', true)
  }
}
