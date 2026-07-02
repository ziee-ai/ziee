import { Info, Bot, Eye, Copy } from 'lucide-react'
import { Card, Tag, Button, Flex, Text, message } from '@/components/ui'
import { Permissions, type HubAssistant } from '@/api-client/types'
import { useState } from 'react'
import { AssistantDetailsDrawer } from '@/modules/hub/modules/assistants/components/AssistantDetailsDrawer'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { useNavigate } from 'react-router-dom'

interface AssistantHubCardProps {
  assistant: HubAssistant
}

export function AssistantHubCard({ assistant }: AssistantHubCardProps) {
  const navigate = useNavigate()
  const [showDetails, setShowDetails] = useState(false)
  const [isCreating, setIsCreating] = useState(false)
  const [isCreatingTemplate, setIsCreatingTemplate] = useState(false)
  const canCreate = usePermission(Permissions.HubAssistantsCreate)
  const canCreateTemplate = usePermission(Permissions.AssistantsTemplateCreate)
  // Templates target a multi-user fleet. On a single-admin desktop
  // (multiUserMode === false) there's no one to template for — hide the
  // "Use as Template" affordance entirely.
  const { multiUserMode } = Stores.AppMode

  // Check if assistant was already created from this hub assistant
  const isAlreadyCreated =
    assistant.created_ids && assistant.created_ids.length > 0
  // Check if a system-wide TEMPLATE already exists for this hub_id
  // (created_by IS NULL). Backend rejects duplicates with 409 as a
  // safety net; the UI uses this to disable the button + show a
  // clearer "Template Installed" label.
  const isAlreadyTemplate =
    assistant.created_template_ids &&
    assistant.created_template_ids.length > 0

  const handleUseAssistant = async () => {
    setIsCreating(true)
    try {
      // Create a user assistant from the hub assistant via store action
      await Stores.HubAssistants.createFromHub({
        hub_id: assistant.name,
        name: assistant.name,
        description: assistant.description,
        instructions: assistant.instructions,
        parameters: assistant.parameters,
        is_default: false,
        enabled: true,
      })

      message.success(
        `Assistant "${assistant.display_name}" created successfully!`,
      )

      // Navigate to the assistants settings page to see the created assistant
      navigate('/settings/assistants')
    } catch (error: any) {
      console.error('Failed to create assistant:', error)
      message.error(
        `Failed to create assistant: ${error.message || 'Unknown error'}`,
      )
    } finally {
      setIsCreating(false)
    }
  }

  const handleUseAsTemplate = async () => {
    setIsCreatingTemplate(true)
    try {
      // Install as a system-wide TEMPLATE (is_template=true, no
      // owner — enforced by the `template_must_have_no_owner` CHECK
      // constraint in migration 6).
      //
      // NOTE: we install with `is_default: false`. The clone-on-signup
      // hook in `assistant::event_handlers` only fans out templates
      // that are BOTH `is_default && enabled`, so this row alone does
      // NOT auto-propagate to new users — the admin must promote it
      // via the templates admin page (a single "Set default" toggle
      // there). We don't default to `is_default=true` here because
      // the assistant repo unsets ALL other template defaults in a
      // single transaction when a new default is set, which would
      // silently bump the existing default off auto-clone duty.
      await Stores.HubAssistants.createTemplateFromHub({
        hub_id: assistant.name,
        name: assistant.name,
        description: assistant.description,
        instructions: assistant.instructions,
        parameters: assistant.parameters,
        is_default: false,
        enabled: true,
      })

      message.success(
        `Template "${assistant.display_name}" installed. \
Mark it as default in /settings/assistant-templates to auto-clone it \
for new users.`,
      )

      // Navigate to the templates admin page so the admin can see it.
      navigate('/settings/assistant-templates')
    } catch (error: any) {
      console.error('Failed to create assistant template:', error)
      message.error(
        `Failed to create template: ${error.message || 'Unknown error'}`,
      )
    } finally {
      setIsCreatingTemplate(false)
    }
  }

  return (
    <>
      <Card
        hoverable
        className="cursor-pointer relative group hover:!shadow-md transition-shadow h-full"
        onClick={() => setShowDetails(true)}
        data-assistant-id={assistant.name}
        data-testid={`hub-assistant-card-${assistant.name}`}
      >
        <div className="flex items-start gap-3 flex-wrap">
          {/* Assistant Info */}
          <div className="flex-1">
            <div className="flex items-center gap-2 mb-2 flex-wrap">
              <div className="flex-1 min-w-48">
                <Flex className="gap-2 items-center">
                  <Bot />
                  <Text className="font-medium cursor-pointer">
                    {assistant.display_name}
                  </Text>
                  {/* v2 per-entry version. Surfaced so admins can spot a
                      catalog bump at a glance — the "Updates" view
                      compares this against the installed entity's
                      `hub_version` per-row, not per-catalog. */}
                  {assistant.version && (
                    <Tag className="text-xs !m-0" data-testid={`hub-assistant-version-tag-${assistant.name}`}>v{assistant.version}</Tag>
                  )}
                  {assistant.category && (
                    <Tag tone="info" className="text-xs" data-testid={`hub-assistant-category-tag-${assistant.name}`}>
                      {assistant.category}
                    </Tag>
                  )}
                  {isAlreadyCreated && <Tag tone="success" data-testid={`hub-assistant-created-tag-${assistant.name}`}>Created</Tag>}
                  {isAlreadyTemplate && (
                    <Tag tone="info" data-testid={`hub-assistant-template-tag-${assistant.name}`}>Template installed</Tag>
                  )}
                </Flex>
              </div>
              <div className="flex flex-wrap gap-1 items-center justify-end">
                <Button
                  icon={<Info />}
                  onClick={e => {
                    e.stopPropagation()
                    setShowDetails(true)
                  }}
                  data-testid={`hub-assistant-details-btn-${assistant.name}`}
                >
                  Details
                </Button>
                {isAlreadyCreated && (
                  <Button
                    icon={<Eye />}
                    onClick={e => {
                      e.stopPropagation()
                      navigate('/settings/assistants')
                    }}
                    data-testid={`hub-assistant-view-btn-${assistant.name}`}
                  >
                    View Assistant
                  </Button>
                )}
                {!isAlreadyCreated && canCreate && (
                  <Button
                    variant="outline"
                    icon={<Bot />}
                    onClick={e => {
                      e.stopPropagation()
                      handleUseAssistant()
                    }}
                    loading={isCreating}
                    disabled={isCreating || isCreatingTemplate}
                    data-testid="hub-assistant-use-btn"
                  >
                    Use Assistant
                  </Button>
                )}
                {/* "Use as Template" — admin power-user action.
                    Shown when the user holds BOTH permissions
                    (`hub::assistants::create` AND
                    `assistant_templates::create`) regardless of
                    whether the per-user "Created" badge is set
                    (a personal install doesn't preclude also
                    installing as a template). Default-styled +
                    distinct `Copy` icon so it's visually
                    separable from the primary "Use Assistant"
                    action. Disabled when a template already
                    exists for this hub_id — the backend rejects
                    duplicates with 409, but disabling here gives
                    the admin clear feedback without a round-trip. */}
                {multiUserMode && canCreate && canCreateTemplate && (
                  <Button
                    icon={<Copy />}
                    onClick={e => {
                      e.stopPropagation()
                      handleUseAsTemplate()
                    }}
                    loading={isCreatingTemplate}
                    disabled={
                      isCreating || isCreatingTemplate || isAlreadyTemplate
                    }
                    data-testid="hub-assistant-use-as-template-btn"
                  >
                    {isAlreadyTemplate ? 'Template Installed' : 'Use as Template'}
                  </Button>
                )}
              </div>
            </div>

            <div>
              {assistant.description && (
                <Text type="secondary" className="text-sm mb-2 block">
                  {assistant.description}
                </Text>
              )}

              {/* Tags */}
              {assistant.tags && assistant.tags.length > 0 && (
                <div className="mb-2">
                  <Text type="secondary" className="text-xs mr-2">
                    Tags:
                  </Text>
                  <Flex
                    wrap
                    className="gap-1 inline-flex"
                  >
                    {assistant.tags.map(tag => (
                      <Tag key={tag} className="text-xs" data-testid={`hub-assistant-card-tag-${assistant.name}-${tag}`}>
                        {tag}
                      </Tag>
                    ))}
                  </Flex>
                </div>
              )}

              {/* Metadata */}
              <div className="mb-2">
                <Flex wrap className="gap-4 text-xs">
                  {assistant.author && (
                    <span>
                      <Text type="secondary" className="text-xs">
                        Author:
                      </Text>{' '}
                      {assistant.author}
                    </span>
                  )}
                </Flex>
              </div>

              {/* v2 Phase 7: dependencies[] replaces recommended_models
                  / recommended_mcp_servers. Show as "Works best with"
                  chips with the reverse-DNS leaf + version range. */}
              {assistant.dependencies &&
                assistant.dependencies.length > 0 && (
                  <div className="mb-2">
                    <Text type="secondary" className="text-xs mr-2">
                      Works best with:
                    </Text>
                    <Flex
                      wrap
                      className="gap-1 inline-flex"
                    >
                      {assistant.dependencies.map(dep => {
                        const leaf = dep.name.split('/').slice(-1)[0]
                        return (
                          <Tag
                            key={`${dep.kind}-${dep.name}`}
                            data-testid={`hub-assistant-card-dep-tag-${assistant.name}-${dep.kind}-${dep.name}`}
                            tone={dep.kind === 'model' ? 'success' : 'info'}
                            className="text-xs"
                          >
                            {leaf} {dep.versionRange}
                          </Tag>
                        )
                      })}
                    </Flex>
                  </div>
                )}
            </div>
          </div>
        </div>
      </Card>

      <AssistantDetailsDrawer
        assistant={showDetails ? assistant : null}
        open={showDetails}
        onClose={() => setShowDetails(false)}
        onUseAssistant={handleUseAssistant}
        onUseAsTemplate={handleUseAsTemplate}
        isCreating={isCreating}
        isCreatingTemplate={isCreatingTemplate}
        isAlreadyCreated={!!isAlreadyCreated}
        isAlreadyTemplate={!!isAlreadyTemplate}
      />
    </>
  )
}
