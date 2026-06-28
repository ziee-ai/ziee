import { useEffect } from 'react'
import { Stores } from '@/core/stores'
import { deriveHiddenSkills } from '@/modules/skill/stores/ConversationSkills.store'
import {
  Button,
  Empty,
  List,
  Spin,
  Switch,
  Text,
  message,
} from '@/components/ui'

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
  const { skills } = Stores.Skill
  const available = Stores.ConversationSkills.available[conversationId]
  const loading = Stores.ConversationSkills.loading[conversationId]

  useEffect(() => {
    Stores.ConversationSkills.loadAvailable(conversationId)
  }, [conversationId])

  // Trigger the install-list load so `skills` is populated.
  useEffect(() => {
    void Stores.Skill.__state.loadSkills()
  }, [])

  if (loading && !available) {
    return <Spin size="sm" label="Loading" />
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
      <Empty description="No skills available in this conversation" data-testid="skill-conversation-empty" />
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
      size="sm"
      data-testid="skill-conversation-list"
      rowKey={skill => skill.id}
      dataSource={allRows}
      renderItem={(skill, index) => {
        const visible = availableIds.has(skill.id)
        return (
          <li
            key={skill.id || index}
            className="flex items-center justify-between py-2"
          >
            <div className="flex-1">
              <Button
                variant="link"
                data-testid={`skill-conversation-open-${skill.id}`}
                className="h-auto p-0 font-medium text-inherit"
                // Thread conversationId so the detail drawer's "Hide in
                // this conversation" checkbox is reachable from chat.
                onClick={() => Stores.SkillDrawer.open(skill, conversationId)}
              >
                {skill.display_name || skill.name}
              </Button>
              {skill.description ? (
                <Text type="secondary" ellipsis>
                  {skill.description}
                </Text>
              ) : null}
            </div>
            <Switch
              size="sm"
              data-testid={`skill-conversation-switch-${skill.id}`}
              checked={visible}
              onChange={next => void handleToggle(skill.id, next)}
            />
          </li>
        )
      }}
    />
  )
}
