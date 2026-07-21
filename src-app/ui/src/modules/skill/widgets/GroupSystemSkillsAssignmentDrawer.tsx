import { useEffect, useState } from 'react'
import { Tag } from '@ziee/kit'
import type { Skill } from '@/api-client/types'
import { Permissions } from '@/api-client/permissions'
import { ApiClient } from '@/api-client'
import { usePermission } from '@/core/permissions'
import { GroupEntityAssignmentDrawer } from '@/components/common/group-entity-assignment/GroupEntityAssignmentDrawer'
import { GroupSystemSkillsAssignment } from '@/modules/skill/widgets/groupSystemSkillsAssignmentDrawer'
import { GroupSystemSkillsWidget } from '@/modules/skill/widgets/groupSystemSkillsWidget'

const skillLabel = (s: Skill) => s.display_name ?? s.name

/**
 * "Assign System Skills" editor drawer on the User Groups page. Binds the
 * shared GroupEntityAssignmentDrawer to the full system-skill list +
 * the group-centric assign endpoints.
 */
export function GroupSystemSkillsAssignmentDrawer() {
  const { isOpen, selectedGroup } = GroupSystemSkillsAssignment
  const canManage = usePermission(Permissions.SkillsAssignToGroups)
  const [allSkills, setAllSkills] = useState<Skill[]>([])

  useEffect(() => {
    if (!isOpen) return
    let cancelled = false
    ApiClient.SkillSystem.list({ limit: 1000, offset: 0 })
      .then(res => {
        if (!cancelled) setAllSkills(Array.isArray(res.skills) ? res.skills : [])
      })
      .catch(err => console.error('Failed to load system skills:', err))
    return () => {
      cancelled = true
    }
  }, [isOpen])

  return (
    <GroupEntityAssignmentDrawer<Skill>
      isOpen={isOpen}
      group={selectedGroup}
      title="Assign System Skills"
      testidPrefix="skill-group-assign"
      canManage={canManage}
      allEntities={allSkills}
      loadAssigned={gid =>
        ApiClient.Group.getSystemSkills({ group_id: gid }).then(r =>
          (Array.isArray(r.skills) ? r.skills : []).map(s => s.id),
        )
      }
      save={(gid, ids) =>
        GroupSystemSkillsWidget.updateGroupSkills(gid, ids)
      }
      onClose={() => GroupSystemSkillsAssignment.closeDrawer()}
      entityLabel={skillLabel}
      emptyText="No system skills available"
      entityBadges={s =>
        s.enabled ? (
          <Tag
            tone="success"
            variant="outline"
            className="text-xs m-0"
            data-testid={`skill-group-assign-status-tag-${s.id}`}
          >
            Enabled
          </Tag>
        ) : (
          <Tag
            tone="warning"
            variant="outline"
            className="text-xs m-0"
            data-testid={`skill-group-assign-status-tag-${s.id}`}
          >
            Disabled
          </Tag>
        )
      }
    />
  )
}
