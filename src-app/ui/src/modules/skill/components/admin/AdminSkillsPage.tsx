import { ImportOutlined } from '@ant-design/icons'
import { Button, Card, Empty, Flex, Space, Typography } from 'antd'
import { useState } from 'react'
import { Permissions } from '@/api-client/types'
import { Can } from '@/core/permissions'
import { Stores } from '@/core/stores'
import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'
import { ImportSkillDialog } from '@/modules/skill/components/ImportSkillDialog'
import { SkillDetailDrawer } from '@/modules/skill/components/SkillDetailDrawer'
import { SkillScopeBadge } from '@/modules/skill/components/SkillScopeBadge'
import { AdminSkillGroupAssignment } from './AdminSkillGroupAssignment'

const { Text } = Typography

/**
 * `/settings/admin/skills` — lists system-scope skills with per-skill
 * group-restriction cards. Admins install system skills from the Hub
 * (scope dropdown on the hub card) or via local import.
 */
export function AdminSkillsPage() {
  const { systemSkills, loading } = Stores.SystemSkill
  const { multiUserMode } = Stores.AppMode
  const [importOpen, setImportOpen] = useState(false)

  return (
    <SettingsPageContainer
      title="System Skills"
      subtitle="Skills installed for the whole deployment"
    >
      <div className="flex flex-col gap-3 h-full">
        <Flex justify="end">
          <Can permission={Permissions.SkillsManageSystem}>
            <Button
              icon={<ImportOutlined />}
              onClick={() => setImportOpen(true)}
            >
              Import
            </Button>
          </Can>
        </Flex>

        {loading && <Text type="secondary">Loading system skills...</Text>}

        <div className="flex flex-col gap-3">
          {systemSkills.map(skill => (
            <Card
              key={skill.id}
              classNames={{ body: '!p-0' }}
              className="overflow-hidden"
              data-skill-id={skill.id}
            >
              <div
                className="p-3 cursor-pointer"
                role="button"
                tabIndex={0}
                onClick={() => Stores.SkillDrawer.open(skill)}
                onKeyDown={e => {
                  if (e.key === 'Enter' || e.key === ' ') {
                    e.preventDefault()
                    Stores.SkillDrawer.open(skill)
                  }
                }}
              >
                <Space vertical size={2}>
                  <Space size={8}>
                    <Text strong>{skill.display_name || skill.name}</Text>
                    <SkillScopeBadge scope={skill.scope} isDev={skill.is_dev} />
                  </Space>
                  {skill.description && (
                    <Text type="secondary" className="text-xs">
                      {skill.description}
                    </Text>
                  )}
                </Space>
              </div>
              {multiUserMode && (
                <AdminSkillGroupAssignment skillId={skill.id} />
              )}
            </Card>
          ))}
        </div>

        {!loading && systemSkills.length === 0 && (
          <Empty description="No system skills installed" className="!mt-12" />
        )}

        <SkillDetailDrawer />
        <ImportSkillDialog
          open={importOpen}
          onClose={() => setImportOpen(false)}
          system
        />
      </div>
    </SettingsPageContainer>
  )
}
