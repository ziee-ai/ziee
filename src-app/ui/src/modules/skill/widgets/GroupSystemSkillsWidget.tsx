import { useEffect } from 'react'
import { BookOpen } from 'lucide-react'
import type { GroupWidgetProps } from '@/modules/user/types/GroupWidget'
import type { Skill } from '@/api-client/types'
import { Permissions } from '@/api-client/permissions'
import { usePermission } from '@/core/permissions'
import { GroupEntityAssignmentWidget } from '@/components/common/group-entity-assignment/GroupEntityAssignmentWidget'
import { GroupSystemSkillsAssignment } from '@/modules/skill/widgets/groupSystemSkillsAssignmentDrawer'
import { GroupSystemSkillsWidget as GroupSystemSkillsWidgetStore } from '@/modules/skill/widgets/groupSystemSkillsWidget'

const skillLabel = (s: Skill) => s.display_name ?? s.name

/**
 * "System Skills" assignment widget on the User Groups page. Thin binding of
 * the shared GroupEntityAssignmentWidget to the skill widget store + drawer.
 */
export function GroupSystemSkillsWidget({ group }: GroupWidgetProps) {
  const data = GroupSystemSkillsWidgetStore.groupSkills.get(group.id)
  const canManage = usePermission(Permissions.SkillsAssignToGroups)

  // The group-system-skills read endpoint requires skills::assign_to_groups
  // (same as canManage). Gate the eager load so a groups::read-only admin
  // without it doesn't 403 on mount.
  useEffect(() => {
    if (canManage) GroupSystemSkillsWidgetStore.loadSkillsForGroup(group.id)
  }, [group.id, canManage])

  return (
    <GroupEntityAssignmentWidget<Skill>
      group={group}
      title="System Skills"
      icon={<BookOpen className="text-primary" aria-hidden="true" />}
      testidPrefix="skill-group-widget"
      canManage={canManage}
      data={
        data
          ? { entities: data.skills, loading: data.loading, error: data.error }
          : undefined
      }
      onEdit={g => GroupSystemSkillsAssignment.openDrawer(g)}
      entityLabel={skillLabel}
      entityActive={s => s.enabled}
    />
  )
}
