import { useEffect, useState } from 'react'
import { Tag } from '@/components/ui'
import type { Skill } from '@/api-client/types'
import { Permissions } from '@/api-client/types'
import { ApiClient } from '@/api-client'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { GroupEntityAssignmentDrawer } from '@/components/common/group-entity-assignment/GroupEntityAssignmentDrawer'

const skillLabel = (s: Skill) => s.display_name ?? s.name

/**
 * "Assign System Skills" editor drawer on the User Groups page. Binds the
 * shared GroupEntityAssignmentDrawer to the full system-skill list +
 * the group-centric assign endpoints.
 */
export function GroupSystemSkillsAssignmentDrawer() {
  const { isOpen, selectedGroup } = Stores.GroupSystemSkillsAssignment
  const canManage = usePermission(Permissions.SkillsAssignToGroups)
  const [allSkills, setAllSkills] = useState<Skill[]>([])

  useEffect(() => {
    if (isOpen) {
      ApiClient.SkillSystem.list({ limit: 1000, offset: 0 })
        .then(res => setAllSkills(res.skills))
        .catch(err => console.error('Failed to load system skills:', err))
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
          r.skills.map(s => s.id),
        )
      }
      save={(gid, ids) =>
        Stores.GroupSystemSkillsWidget.updateGroupSkills(gid, ids)
      }
      onClose={() => Stores.GroupSystemSkillsAssignment.closeDrawer()}
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
