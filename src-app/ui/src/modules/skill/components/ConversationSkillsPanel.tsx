import { useEffect } from 'react'
import { Stores } from '@ziee/framework/stores'
import { deriveHiddenSkills } from '@/modules/skill/stores'
import {
  Alert,
  Button,
  Empty,
  List,
  Spin,
  Switch,
  Text,
  message,
} from '@ziee/kit'
import { SkillDrawer } from '@/modules/skill/stores/skillDrawer'
import { Skill } from '@/modules/skill/stores/skill'

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
  const { skills } = Skill
  const available = Stores.ConversationSkills.available[conversationId]
  const loading = Stores.ConversationSkills.loading[conversationId]
  const error = Stores.ConversationSkills.error


  useEffect(() => {
    Stores.ConversationSkills.loadAvailable(conversationId)
  }, [conversationId])

  // NOTE: no manual loadSkills() here — reading `Skill.skills`
  // above self-initializes the install list via the store's
  // `__init__.skills` hook (and `sync:skill` keeps it fresh), so a
  // mount-time fetch would be redundant (REACT_COMPONENT_PATTERNS:
  // don't manually load in useEffect).

  if (loading && !available) {
    return <Spin size="sm" label="Loading" />
  }

  // A load failure leaves `available` undefined; surface it instead of falling
  // through to a misleading empty panel.
  if (error && !available) {
    return (
      <Alert
        data-testid="conversation-skills-load-error-alert"
        tone="error"
        title="Failed to load skills"
        description={error}
      />
    )
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
    <div className="max-h-[60vh] overflow-y-auto">
    <List
      size="sm"
      data-testid="skill-conversation-list"
      rowKey={skill => skill.id}
      dataSource={allRows}
      renderItem={(skill, index) => {
        const visible = availableIds.has(skill.id)
        // NOTE: the `List` kit wraps each renderItem in its own <li>, so this
        // returns a <div> — a nested <li> here is an invalid-DOM hydration error.
        return (
          <div
            key={skill.id || index}
            className="flex items-center justify-between py-2"
          >
            {/* min-w-0 lets this flex child shrink below its content so the
                description below can truncate instead of overrunning the
                Switch; the name sits in its own block so the description
                drops to the next line and ellipsis-clips within the bound. */}
            <div className="flex-1 min-w-0">
              <div>
                <Button
                  variant="link"
                  data-testid={`skill-conversation-open-${skill.id}`}
                  className="h-auto max-w-full p-0 font-medium text-inherit"
                  // Thread conversationId so the detail drawer's "Hide in
                  // this conversation" checkbox is reachable from chat.
                  onClick={() => SkillDrawer.open(skill, conversationId)}
                >
                  <span className="truncate">
                    {skill.display_name || skill.name}
                  </span>
                </Button>
              </div>
              {skill.description ? (
                <Text type="secondary" ellipsis>
                  {skill.description}
                </Text>
              ) : null}
            </div>
            <Switch
              tooltip="Toggle this skill for the conversation"
              size="sm"
              className="shrink-0"
              data-testid={`skill-conversation-switch-${skill.id}`}
              checked={visible}
              onChange={next => void handleToggle(skill.id, next)}
            />
          </div>
        )
      }}
    />
    </div>
  )
}
