import { Import as ImportIcon } from 'lucide-react'
import { useState } from 'react'
import { Button, Card, Empty, Flex, Text } from '@/components/ui'
import { ListPagination } from '@/components/common/ListPagination'
import { Permissions } from '@/api-client/types'
import { Can } from '@/core/permissions'
import { Stores } from '@/core/stores'
import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'
import { ImportSkillDialog } from './ImportSkillDialog'
import { SkillDetailDrawer } from './SkillDetailDrawer'
import { SkillScopeBadge } from './SkillScopeBadge'

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
  const [page, setPage] = useState(1)
  const [pageSize, setPageSize] = useState(10)

  // Client-side pagination — the skills list endpoint returns the full set (no
  // server total). Clamp the page so deletions can't strand the view on an
  // empty page.
  const totalPages = Math.max(1, Math.ceil(skills.length / pageSize))
  const currentPage = Math.min(page, totalPages)
  const pagedSkills = skills.slice(
    (currentPage - 1) * pageSize,
    currentPage * pageSize,
  )

  return (
    <SettingsPageContainer
      title="Skills"
      subtitle="Reusable knowledge bundles the assistant can load on demand"
    >
      <div className="flex flex-col gap-3" data-testid="skills-page">
        <Flex justify="end">
          <Can permission={Permissions.SkillsInstall}>
            <Button
              icon={<ImportIcon />}
              data-testid="skill-list-import-button"
              onClick={() => setImportOpen(true)}
            >
              Import
            </Button>
          </Can>
        </Flex>

        {loading && <Text type="secondary">Loading skills...</Text>}

        <div className="flex flex-col gap-3">
          {pagedSkills.map(skill => (
            <Card
              key={skill.id}
              hoverable
              size="sm"
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
              data-testid={`skill-list-card-${skill.id}`}
              className="cursor-pointer"
            >
              <Flex justify="between" align="start" className="gap-3">
                {/* flex-1 min-w-0 (not an inline-flex Space, which is
                    shrink-to-fit and grows the card past the viewport). */}
                <div className="flex flex-col gap-1 min-w-0 flex-1">
                  <div className="flex items-center gap-2 flex-wrap min-w-0">
                    <Text strong>{skill.display_name || skill.name}</Text>
                    <SkillScopeBadge scope={skill.scope} isDev={skill.is_dev} />
                  </div>
                  {skill.description && (
                    <Text type="secondary" className="text-xs" ellipsis>
                      {skill.description}
                    </Text>
                  )}
                </div>
              </Flex>
            </Card>
          ))}
        </div>

        {skills.length > 0 && (
          <ListPagination
            data-testid="skill-list-pagination"
            current={currentPage}
            total={skills.length}
            pageSize={pageSize}
            onChange={setPage}
            onPageSizeChange={(size) => {
              setPageSize(size)
              setPage(1)
            }}
            aria-label="Skills pagination"
          />
        )}

        {!loading && skills.length === 0 && (
          <Empty
            description="No skills installed yet — browse the Hub to install one"
            className="!mt-12"
            data-testid="skill-list-empty"
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
