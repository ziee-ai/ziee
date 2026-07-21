import { useEffect } from 'react'
import { ApiClient } from '@/api-client'
import { Permissions } from '@/api-client/permissions'
import { usePermission } from '@/core/permissions'
import { UserGroupAssignment } from '@/components/common/UserGroupAssignment'
import { SystemSkill } from '@/modules/skill/stores/systemSkill'

interface AdminSkillGroupAssignmentProps {
  skillId: string
}

/**
 * Group-assignment section for a system skill (empty assignment = available to
 * ALL users). Thin wrapper over the shared UserGroupAssignment component.
 */
export function AdminSkillGroupAssignment({
  skillId,
}: AdminSkillGroupAssignmentProps) {
  const entry = SystemSkill.groups[skillId]
  const assignedIds = entry?.groupIds ?? []
  const loading = entry?.loading ?? false
  const canAssign = usePermission(Permissions.SkillsAssignToGroups)

  useEffect(() => {
    // Effect context → use `.$` (the `Stores.X.*` proxy is render-only).
    const state = SystemSkill.$
    if (state.groups[skillId]) return
    void state.loadGroups(skillId)
  }, [skillId])

  return (
    // px-3 aligns the section with the card's p-3 header (the card is a plain
    // bordered div with no content padding of its own).
    <div data-skill-id={skillId} className="px-3">
      <UserGroupAssignment
        data-testid="skill-group"
        assignedGroups={assignedIds.map(id => ({ id, name: id }))}
        loading={loading}
        canAssign={canAssign}
        emptyText="Available to all users"
        editor={{
          loadAllGroups: async () => {
            const res = await ApiClient.UserGroup.list({ page: 1, per_page: 100 })
            return res.groups.map(g => ({ id: g.id, name: g.name, description: g.description, is_default: g.is_default }))
          },
          save: ids => SystemSkill.setGroups(skillId, ids),
        }}
      />
    </div>
  )
}
