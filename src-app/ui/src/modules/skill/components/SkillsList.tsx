import { ImportOutlined, ReadOutlined } from '@ant-design/icons'
import { Button, Card, Empty, Flex, Space, Typography } from 'antd'
import { useState } from 'react'
import { Permissions } from '@/api-client/types'
import { Can } from '@/core/permissions'
import { Stores } from '@/core/stores'
import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'
import { ImportSkillDialog } from './ImportSkillDialog'
import { SkillDetailDrawer } from './SkillDetailDrawer'
import { SkillScopeBadge } from './SkillScopeBadge'

const { Text } = Typography

/**
 * `/skills` page — lists the user's own + accessible system skills,
 * each tagged with a scope badge. Clicking a card opens the detail
 * drawer (SKILL.md body + per-conversation hide checkbox when in a
 * conversation context). Skills self-install via the hub; this page is
 * the read/manage surface.
 */
export function SkillsList() {
  const { skills, loading } = Stores.Skill
  const [importOpen, setImportOpen] = useState(false)

  return (
    <SettingsPageContainer
      title="Skills"
      subtitle="Reusable knowledge bundles the assistant can load on demand"
    >
      <div className="flex flex-col gap-3 h-full">
        <Flex justify="end">
          <Can permission={Permissions.SkillsInstall}>
            <Button
              icon={<ImportOutlined />}
              onClick={() => setImportOpen(true)}
            >
              Import
            </Button>
          </Can>
        </Flex>

        {loading && <Text type="secondary">Loading skills...</Text>}

        <div className="flex flex-col gap-3">
          {skills.map(skill => (
            <Card
              key={skill.id}
              hoverable
              size="small"
              role="button"
              tabIndex={0}
              onClick={() => Stores.SkillDrawer.open(skill)}
              onKeyDown={e => {
                if (e.key === 'Enter' || e.key === ' ') {
                  e.preventDefault()
                  Stores.SkillDrawer.open(skill)
                }
              }}
              data-skill-id={skill.id}
            >
              <Flex justify="space-between" align="flex-start" gap={12}>
                <Space vertical size={2} className="min-w-0">
                  <Space size={8}>
                    <ReadOutlined />
                    <Text strong>{skill.display_name || skill.name}</Text>
                    <SkillScopeBadge scope={skill.scope} isDev={skill.is_dev} />
                  </Space>
                  {skill.description && (
                    <Text type="secondary" className="text-xs" ellipsis>
                      {skill.description}
                    </Text>
                  )}
                </Space>
              </Flex>
            </Card>
          ))}
        </div>

        {!loading && skills.length === 0 && (
          <Empty
            description="No skills installed yet — browse the Hub to install one"
            className="!mt-12"
          />
        )}

        <SkillDetailDrawer />
        <ImportSkillDialog
          open={importOpen}
          onClose={() => setImportOpen(false)}
        />
      </div>
    </SettingsPageContainer>
  )
}
