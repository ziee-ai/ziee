import { Import } from 'lucide-react'
import { Button, Empty, ErrorState, Flex, Text } from '@ziee/kit'
import { ListPagination } from '@/components/common/ListPagination'
import { useEffect, useState } from 'react'
import { Permissions } from '@/api-client/permissions'
import { Can } from '@/core/permissions'
import { Stores } from '@ziee/framework/stores'
import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'
import { ImportSkillDialog } from '@/modules/skill/components/ImportSkillDialog'
import { SkillDetailDrawer } from '@/modules/skill/components/SkillDetailDrawer'
import { SkillScopeBadge } from '@/modules/skill/components/SkillScopeBadge'
import { AdminSkillGroupAssignment } from './AdminSkillGroupAssignment'

/**
 * `/settings/admin/skills` — lists system-scope skills with per-skill
 * group-restriction cards. Admins install system skills from the Hub
 * (scope dropdown on the hub card) or via local import.
 */
export function AdminSkillsPage() {
  const { systemSkills, loading, error } = Stores.SystemSkill
  const { multiUserMode } = Stores.AppMode
  const [importOpen, setImportOpen] = useState(false)

  // Client-side pagination (the store loads the full list).
  const [page, setPage] = useState(1)
  const [pageSize, setPageSize] = useState(10)
  const total = systemSkills.length
  useEffect(() => {
    const maxPage = Math.max(1, Math.ceil(total / pageSize))
    if (page > maxPage) setPage(maxPage)
  }, [total, pageSize, page])
  const pagedSkills = systemSkills.slice((page - 1) * pageSize, page * pageSize)

  return (
    <SettingsPageContainer
      title="System Skills"
      subtitle="Skills installed for the whole deployment"
    >
      <div className="flex flex-col gap-3" data-testid="skills-admin-page">
        <Flex justify="end">
          <Can permission={Permissions.SkillsManageSystem}>
            <Button
              icon={<Import />}
              data-testid="skill-admin-import-button"
              onClick={() => setImportOpen(true)}
            >
              Import
            </Button>
          </Can>
        </Flex>

        {loading && !error && (
          <Text type="secondary">Loading system skills...</Text>
        )}

        <div className="flex flex-col gap-3">
          {pagedSkills.map(skill => (
            // Plain bordered div (not kit Card) so the only padding is the inner
            // p-3 header — a Card would add its own px-6/py-4 on top (double pad).
            <div
              key={skill.id}
              className="overflow-hidden border rounded-lg"
              data-skill-id={skill.id}
              data-testid={`skill-admin-card-${skill.id}`}
            >
              <div
                className="p-3 cursor-pointer focus-visible:outline focus-visible:outline-2"
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
                <div className="flex flex-col gap-2">
                  <div className="flex items-center gap-2">
                    <Text strong>{skill.display_name || skill.name}</Text>
                    <SkillScopeBadge scope={skill.scope} isDev={skill.is_dev} />
                  </div>
                  {skill.description && (
                    <Text type="secondary" className="text-xs">
                      {skill.description}
                    </Text>
                  )}
                </div>
              </div>
              {multiUserMode && (
                <AdminSkillGroupAssignment skillId={skill.id} />
              )}
            </div>
          ))}
        </div>

        {error && systemSkills.length === 0 ? (
          <ErrorState
            resource="system skills"
            description="Something went wrong while loading system skills."
            details={error}
            onRetry={() => Stores.SystemSkill.loadSystemSkills()}
            data-testid="skill-admin-error"
          />
        ) : (
          !loading &&
          systemSkills.length === 0 && (
            <Empty description="No system skills installed" className="!mt-12" data-testid="skill-admin-empty" />
          )
        )}

        {total > 0 && (
          <ListPagination
            data-testid="skill-admin-pagination"
            current={page}
            total={total}
            pageSize={pageSize}
            onChange={(p: number) => setPage(p)}
            onPageSizeChange={(size: number) => { setPageSize(size); setPage(1) }}
            itemNoun="skills"
            aria-label="System skills pagination"
          />
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
