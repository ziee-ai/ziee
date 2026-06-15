import { DeleteOutlined } from '@ant-design/icons'
import {
  App,
  Button,
  Checkbox,
  Descriptions,
  Drawer,
  Popconfirm,
  Space,
  Typography,
} from 'antd'
import { useEffect, useMemo, useState } from 'react'
import { Streamdown } from 'streamdown'
import { ApiClient } from '@/api-client'
import type { Skill } from '@/api-client/types'
import { Permissions } from '@/api-client/types'
import { usePermission } from '@/core/permissions'
import { Stores } from '@/core/stores'
import { StreamdownErrorBoundary } from '@/modules/chat/core/utils/StreamdownErrorBoundary'
import { SkillScopeBadge } from './SkillScopeBadge'

const { Text, Title } = Typography

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
  const { message } = App.useApp()
  const { isOpen, skill, conversationId } = Stores.SkillDrawer
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
  // `__state.available` (non-reactive) meant the checkbox stayed false
  // for an actually-hidden skill if `available` wasn't loaded yet.
  const availableMap = Stores.ConversationSkills.available
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
      void Stores.ConversationSkills.__state.loadAvailable(conversationId)
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
        onClose={() => Stores.SkillDrawer.close()}
        closable={{ closeIcon: true }}
        size="large"
      />
    )
  }

  const editable = skill.scope === 'system' ? canManageSystem : canManage

  const handleToggleHidden = async (next: boolean) => {
    if (!conversationId) return
    try {
      if (next) {
        await Stores.ConversationSkills.hide(skill.id, conversationId)
      } else {
        await Stores.ConversationSkills.unhide(skill.id, conversationId)
      }
      setHidden(next)
    } catch {
      message.error('Failed to update conversation visibility')
    }
  }

  const handleDelete = async () => {
    try {
      if (skill.scope === 'system') {
        await Stores.SystemSkill.deleteSystemSkill(skill.id)
      } else {
        await Stores.Skill.deleteSkill(skill.id)
      }
      message.success('Skill deleted')
      Stores.SkillDrawer.close()
    } catch {
      message.error('Failed to delete skill')
    }
  }

  return (
    <Drawer
      open={isOpen}
      onClose={() => Stores.SkillDrawer.close()}
      closable={{ closeIcon: true }}
      size="large"
      title={
        <Space>
          <Title level={5} className="!m-0">
            {skill.display_name || skill.name}
          </Title>
          <SkillScopeBadge scope={skill.scope} isDev={skill.is_dev} />
        </Space>
      }
      extra={
        editable ? (
          <Popconfirm
            title="Delete this skill?"
            description="This removes the skill and its extracted files."
            onConfirm={handleDelete}
            okText="Delete"
            okButtonProps={{ danger: true }}
          >
            <Button danger size="small" icon={<DeleteOutlined />}>
              Delete
            </Button>
          </Popconfirm>
        ) : null
      }
    >
      <div className="flex flex-col gap-4">
        <Descriptions size="small" column={1} bordered>
          <Descriptions.Item label="Name">{skill.name}</Descriptions.Item>
          {skill.version && (
            <Descriptions.Item label="Version">
              {skill.version}
            </Descriptions.Item>
          )}
          <Descriptions.Item label="Files">
            {skill.file_count}
          </Descriptions.Item>
          <Descriptions.Item label="Size">
            {(skill.bundle_size_bytes / 1024).toFixed(1)} KiB
          </Descriptions.Item>
        </Descriptions>

        {conversationId && (
          <Checkbox
            checked={hidden}
            onChange={e => void handleToggleHidden(e.target.checked)}
          >
            Hide in this conversation
          </Checkbox>
        )}

        <div className="overflow-auto">
          <StreamdownErrorBoundary fallbackText={markdown}>
            <Streamdown shikiTheme={['github-light', 'github-dark']}>
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
          <Text type="secondary" className="text-xs">
            Couldn’t load skill content.
          </Text>
        )}
        {body && (
          <div className="overflow-auto">
            <Title level={5}>Skill content (SKILL.md)</Title>
            <StreamdownErrorBoundary fallbackText={body}>
              <Streamdown shikiTheme={['github-light', 'github-dark']}>
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
