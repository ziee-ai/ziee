import { Trash2 } from 'lucide-react'
import {
  Button,
  Checkbox,
  Descriptions,
  Confirm,
  Space,
  Text,
  Title,
  message,
} from '@ziee/kit'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { useEffect, useMemo, useState } from 'react'
import { Streamdown } from '@/modules/chat/core/utils/LazyStreamdown'
import { ApiClient } from '@/api-client'
import type { Skill } from '@/api-client/types'
import { Permissions } from '@/api-client/permissions'
import { usePermission } from '@/core/permissions'
import { StreamdownErrorBoundary } from '@/modules/chat/core/utils/StreamdownErrorBoundary'
import { SkillScopeBadge } from './SkillScopeBadge'
import { SystemSkill } from '@/modules/skill/stores/systemSkill'
import { SkillDrawer } from '@/modules/skill/stores/skillDrawer'
import { Skill as SkillStore } from '@/modules/skill/stores/skill'
import { ConversationSkills } from '@/modules/skill/stores/conversationSkills'

/** Build a readable markdown summary from the skill's persisted
 *  metadata. This renders the parsed FRONTMATTER only (`description`,
 *  `when_to_use`) — which is what the model sees in the available-skills
 *  listing — NOT the full SKILL.md body.
 *
 *  The frontmatter-derived summary (title / description / when-to-use)
 *  is rendered immediately from the `Skill` row; the full SKILL.md body
 *  is fetched on open via `GET /api/skills/{id}/body` (the on-disk
 *  bundle body, frontmatter stripped) and rendered below it. */
function buildSkillMarkdown(skill: Skill): string {
  const parts: string[] = []
  const title = skill.display_name || skill.name
  parts.push(`# ${title}`)
  if (skill.description) parts.push(skill.description)
  if (skill.when_to_use) {
    parts.push(`## When to use\n\n${skill.when_to_use}`)
  }
  return parts.join('\n\n')
}

export function SkillDetailDrawer() {
  const { isOpen, skill, conversationId } = SkillDrawer
  // No dedicated `skills::manage` permission is generated; a user can
  // manage their OWN user-scope skills (any installer can), while
  // system skills require `skills::manage_system`.
  const canManage = usePermission(Permissions.SkillsInstall)
  const canManageSystem = usePermission(Permissions.SkillsManageSystem)
  const [hidden, setHidden] = useState(false)
  const [body, setBody] = useState<string | null>(null)
  const [bodyLoading, setBodyLoading] = useState(false)
  const [bodyError, setBodyError] = useState(false)

  // Read the REACTIVE available map so the effect below re-runs when
  // the conversation's available listing loads — reading
  // `$.available` (non-reactive) meant the checkbox stayed false
  // for an actually-hidden skill if `available` wasn't loaded yet.
  const availableMap = ConversationSkills.available
  const conversationAvailable = conversationId
    ? availableMap[conversationId]
    : undefined

  useEffect(() => {
    // Re-sync the checkbox each time a (different) skill opens or the
    // available listing loads. The effective hide state comes from the
    // conversation-skills store's available listing — a skill missing
    // from "available" is hidden. If the listing isn't loaded yet,
    // trigger a fetch (the reactive `conversationAvailable` dep then
    // re-runs this effect once it arrives).
    if (!isOpen || !skill || !conversationId) return
    if (conversationAvailable) {
      setHidden(!conversationAvailable.some(s => s.id === skill.id))
    } else {
      void ConversationSkills.loadAvailable(conversationId)
    }
  }, [isOpen, skill, conversationId, conversationAvailable])

  useEffect(() => {
    // Fetch the full SKILL.md body (frontmatter stripped) from the
    // on-disk bundle when a skill opens. Reset between skills.
    let cancelled = false
    setBody(null)
    setBodyError(false)
    if (isOpen && skill) {
      setBodyLoading(true)
      ApiClient.Skill.getBody({ id: skill.id })
        .then(res => {
          if (!cancelled) setBody(res.body)
        })
        .catch(() => {
          // Surface a subtle error state rather than rendering blank.
          if (!cancelled) {
            setBody(null)
            setBodyError(true)
          }
        })
        .finally(() => {
          if (!cancelled) setBodyLoading(false)
        })
    }
    return () => {
      cancelled = true
    }
  }, [isOpen, skill])

  const markdown = useMemo(
    () => (skill ? buildSkillMarkdown(skill) : ''),
    [skill],
  )

  if (!skill) {
    return (
      <Drawer
        open={isOpen}
        data-testid="skill-detail-sheet"
        onClose={() => SkillDrawer.close()}
        title="Skill details"
      />
    )
  }

  // Built-in capability skills are owned by the binary — never editable /
  // deletable from the UI (the backend rejects it too).
  const editable =
    skill.scope === 'built_in'
      ? false
      : skill.scope === 'system'
        ? canManageSystem
        : canManage

  const handleToggleHidden = async (next: boolean) => {
    if (!conversationId) return
    try {
      if (next) {
        await ConversationSkills.hide(skill.id, conversationId)
      } else {
        await ConversationSkills.unhide(skill.id, conversationId)
      }
      setHidden(next)
    } catch {
      message.error('Failed to update conversation visibility')
    }
  }

  const handleDelete = async () => {
    try {
      if (skill.scope === 'system') {
        await SystemSkill.deleteSystemSkill(skill.id)
      } else {
        await SkillStore.deleteSkill(skill.id)
      }
      message.success('Skill deleted')
      SkillDrawer.close()
    } catch {
      message.error('Failed to delete skill')
    }
  }

  return (
    <Drawer
      open={isOpen}
      data-testid="skill-detail-sheet-loaded"
      onClose={() => SkillDrawer.close()}
      title={
        <Space>
          <Title level={5} className="!m-0">
            {skill.display_name || skill.name}
          </Title>
          <SkillScopeBadge scope={skill.scope} isDev={skill.is_dev} />
        </Space>
      }
      footer={
        editable ? (
          <div className="flex justify-end">
            <Confirm
              data-testid="skill-delete-confirm"
              title="Delete this skill?"
              description="This removes the skill and its extracted files."
              onConfirm={handleDelete}
              okText="Delete"
              cancelText="Cancel"
              okButtonProps={{ danger: true }}
            >
              <Button variant="ghost" size="default" data-testid="skill-delete-button" icon={<Trash2 />}>
                Delete
              </Button>
            </Confirm>
          </div>
        ) : null
      }
    >
      <div className="flex flex-col gap-4">
        <Descriptions
          size="sm"
          column={1}
          bordered
          data-testid="skill-detail-descriptions"
          items={[
            { key: 'name', label: 'Name', children: skill.name },
            ...(skill.version
              ? [{ key: 'version', label: 'Version', children: skill.version }]
              : []),
            { key: 'files', label: 'Files', children: skill.file_count },
            {
              key: 'size',
              label: 'Size',
              children: `${(skill.bundle_size_bytes / 1024).toFixed(1)} KiB`,
            },
          ]}
        />

        {conversationId && (
          <Checkbox
            checked={hidden}
            data-testid="skill-detail-hide-checkbox"
            onChange={(next: boolean) => void handleToggleHidden(next)}
            label="Hide in this conversation"
          />
        )}

        <div className="overflow-auto">
          <StreamdownErrorBoundary fallbackText={markdown}>
            <Streamdown variant="base">
              {markdown}
            </Streamdown>
          </StreamdownErrorBoundary>
        </div>

        {/* Full SKILL.md body fetched from the on-disk bundle. */}
        {bodyLoading && (
          <Text type="secondary" className="text-xs">
            Loading skill content…
          </Text>
        )}
        {bodyError && !bodyLoading && (
          <Text
            type="secondary"
            className="text-xs"
            data-testid="skill-detail-body-error"
          >
            Couldn't load skill content.
          </Text>
        )}
        {body && (
          <div className="overflow-auto" data-testid="skill-detail-body">
            <Title level={5}>Skill content (SKILL.md)</Title>
            <StreamdownErrorBoundary fallbackText={body}>
              <Streamdown variant="base">
                {body}
              </Streamdown>
            </StreamdownErrorBoundary>
          </div>
        )}

        <div>
          <Text type="secondary" className="text-xs">
            Extracted at {skill.extracted_path}
          </Text>
        </div>
      </div>
    </Drawer>
  )
}
