import { Card } from '@ziee/kit'
import { useEffect } from 'react'
import { useParams } from 'react-router-dom'
import { ApiClient } from '@/api-client'
import { Permissions } from '@/api-client/permissions'
import { usePermission } from '@/core/permissions'
import { UserGroupAssignment } from '@/components/common/UserGroupAssignment'
import { emitLlmProviderGroupsChanged } from '@/modules/llm-provider/events'
import { ProviderGroupAssignmentCard as ProviderGroupAssignmentCardStore } from '@/modules/llm-provider/components/providerGroupAssignmentCard'
import { LlmProvider } from '@/modules/llm-provider/stores/llmProvider'

/**
 * Card for managing which user groups have access to an LLM provider. Thin
 * wrapper over the shared UserGroupAssignment; Assign opens the shared editor
 * Drawer, save diffs the selection via assign/removeGroupToProvider.
 */
export function ProviderGroupAssignmentCard() {
  const { providerId } = useParams<{ providerId?: string }>()
  const { providerGroups } = ProviderGroupAssignmentCardStore
  const providerData = providerId ? providerGroups.get(providerId) : undefined
  // Assigning providers to groups requires llm_providers::assign_groups (the
  // assign/remove/update endpoints enforce it). Hoisted ABOVE the early return
  // below so the hook count stays stable across providerId toggles.
  const canAssign = usePermission(Permissions.LlmProvidersAssignGroups)

  useEffect(() => {
    if (providerId) {
      ProviderGroupAssignmentCardStore.loadGroupsForProvider(providerId)
    }
  }, [providerId])

  if (!providerId) return null
  const pid = providerId

  return (
    <Card data-testid="llm-provider-groups-card">
      <UserGroupAssignment
        data-testid="llm-provider-groups"
        assignedGroups={(providerData?.groups ?? []).map(g => ({ id: g.id, name: g.name }))}
        loading={providerData?.loading}
        // A viewer with only llm_providers::read must not see the Assign
        // affordance (it 403s). Mirrors McpServerGroupsAssignmentCard.
        canAssign={canAssign}
        emptyText="No groups assigned"
        editor={{
          loadAllGroups: async () => {
            const res = await ApiClient.UserGroup.list({ page: 1, per_page: 100 })
            return res.groups.map(g => ({ id: g.id, name: g.name, description: g.description, is_default: g.is_default }))
          },
          save: async ids => {
            const currentGroups = await LlmProvider.getGroupsForProvider(pid)
            const currentIds = new Set(currentGroups.map(g => g.id))
            const newIds = new Set(ids)
            for (const gid of ids.filter(id => !currentIds.has(id))) {
              await LlmProvider.assignGroupToProvider(pid, gid)
            }
            for (const gid of [...currentIds].filter(id => !newIds.has(id))) {
              await LlmProvider.removeGroupFromProvider(pid, gid)
            }
            await emitLlmProviderGroupsChanged(pid, ids)
          },
        }}
      />
    </Card>
  )
}
