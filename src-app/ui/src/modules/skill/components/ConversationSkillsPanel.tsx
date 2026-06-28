import { Alert, App, Empty, List, Spin, Switch, Typography } from 'antd'
import { useEffect } from 'react'
import { Stores } from '@/core/stores'
import { deriveHiddenSkills } from '@/modules/skill/stores/ConversationSkills.store'

const { Text } = Typography

interface ConversationSkillsPanelProps {
  conversationId: string
}

/**
 * Per-conversation opt-out panel. Lists every skill available to the
 * user; a toggle controls whether each one is visible to the model in
 * THIS conversation (Path B — default available, hide per conversation).
 */
export function ConversationSkillsPanel({
  conversationId,
}: ConversationSkillsPanelProps) {
  const { message } = App.useApp()
  const { skills } = Stores.Skill
  const available = Stores.ConversationSkills.available[conversationId]
  const loading = Stores.ConversationSkills.loading[conversationId]
  const error = Stores.ConversationSkills.error

  useEffect(() => {
    Stores.ConversationSkills.loadAvailable(conversationId)
  }, [conversationId])

  // Trigger the install-list load so `skills` is populated.
  useEffect(() => {
    void Stores.Skill.__state.loadSkills()
  }, [])

  if (loading && !available) {
    return <Spin size="small" />
  }

  // A load failure leaves `available` undefined; surface it instead of falling
  // through to a misleading empty panel. (hide/unhide errors set `error` too,
  // but those paths always have `available` already loaded, so they don't hit
  // this branch.)
  if (error && !available) {
    return <Alert type="error" showIcon message="Failed to load skills" description={error} />
  }

  const availableIds = new Set((available ?? []).map(s => s.id))
  const hidden = deriveHiddenSkills(skills, available)
  const hiddenIds = new Set(hidden.map(s => s.id))
  // Union of visible + hidden skills the user owns/accesses.
  const allRows = skills.filter(
    s => s.enabled && (availableIds.has(s.id) || hiddenIds.has(s.id)),
  )

  if (allRows.length === 0) {
    return (
      <Empty
        description="No skills available in this conversation"
        image={Empty.PRESENTED_IMAGE_SIMPLE}
      />
    )
  }

  const handleToggle = async (skillId: string, visible: boolean) => {
    try {
      if (visible) {
        await Stores.ConversationSkills.unhide(skillId, conversationId)
      } else {
        await Stores.ConversationSkills.hide(skillId, conversationId)
      }
    } catch {
      message.error('Failed to update skill visibility')
    }
  }

  return (
    <List
      size="small"
      dataSource={allRows}
      renderItem={skill => {
        const visible = availableIds.has(skill.id)
        return (
          <List.Item
            actions={[
              <Switch
                key="toggle"
                size="small"
                checked={visible}
                onChange={next => void handleToggle(skill.id, next)}
              />,
            ]}
          >
            <List.Item.Meta
              title={
                <button
                  type="button"
                  className="bg-transparent border-0 p-0 cursor-pointer text-left text-inherit"
                  // Thread conversationId so the detail drawer's "Hide in
                  // this conversation" checkbox is reachable from chat.
                  onClick={() => Stores.SkillDrawer.open(skill, conversationId)}
                >
                  {skill.display_name || skill.name}
                </button>
              }
              description={
                skill.description ? (
                  <Text type="secondary" className="text-xs" ellipsis>
                    {skill.description}
                  </Text>
                ) : undefined
              }
            />
          </List.Item>
        )
      }}
    />
  )
}
