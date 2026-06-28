import { Copy, Bot } from 'lucide-react'
import { Sheet } from '@/components/ui'
import { Button, Flex, Tag, Text, Title, Card } from '@/components/ui'
import { Permissions, type HubAssistant } from '@/api-client/types'
import { usePermission } from '@/core/permissions'

interface AssistantDetailsDrawerProps {
  assistant: HubAssistant | null
  open: boolean
  onClose: () => void
  /** Forwarded from the parent card — invoked when the user clicks
   *  the drawer-footer install button. The parent owns the loading
   *  state + toast feedback + navigation. Passing handlers in lets
   *  the drawer stay stateless and lets the card remain the single
   *  source of truth for "what's currently installing." */
  onUseAssistant?: () => void
  onUseAsTemplate?: () => void
  isCreating?: boolean
  isCreatingTemplate?: boolean
  isAlreadyCreated?: boolean
  isAlreadyTemplate?: boolean
}

export function AssistantDetailsDrawer({
  assistant,
  open,
  onClose,
  onUseAssistant,
  onUseAsTemplate,
  isCreating = false,
  isCreatingTemplate = false,
  isAlreadyCreated = false,
  isAlreadyTemplate = false,
}: AssistantDetailsDrawerProps) {
  const canCreate = usePermission(Permissions.HubAssistantsCreate)
  const canCreateTemplate = usePermission(Permissions.AssistantsTemplateCreate)

  if (!assistant) return null

  // Footer install actions — same gating as the card so a user who
  // can't act on a button doesn't see it. The handlers are optional
  // (a hub catalog that's not yet wired up wouldn't pass them), so
  // we render no footer when neither handler is supplied.
  const footer =
    onUseAssistant || onUseAsTemplate ? (
      <Flex justify="end" gap="small">
        {!isAlreadyCreated && canCreate && onUseAssistant && (
          <Button
            variant="default"
            icon={<Bot />}
            loading={isCreating}
            disabled={isCreating || isCreatingTemplate}
            onClick={onUseAssistant}
            data-testid="hub-assistant-drawer-use-btn"
          >
            Use Assistant
          </Button>
        )}
        {canCreate && canCreateTemplate && onUseAsTemplate && (
          <Button
            icon={<Copy />}
            loading={isCreatingTemplate}
            disabled={
              isCreating || isCreatingTemplate || isAlreadyTemplate
            }
            onClick={onUseAsTemplate}
            data-testid="hub-assistant-drawer-use-as-template-btn"
          >
            {isAlreadyTemplate ? 'Template Installed' : 'Use as Template'}
          </Button>
        )}
      </Flex>
    ) : undefined

  return (
    <Sheet
      title={assistant.display_name}
      open={open}
      onOpenChange={(v) => { if (!v) onClose() }}
      footer={footer}
    >
      <Flex direction="column" className="gap-4">
        {/* Basic Info */}
        <div>
          <Title level={3} className="!m-0 !mb-2">
            {assistant.display_name}
          </Title>
          <Text type="secondary" className="text-xs">
            {assistant.name}
          </Text>
          {assistant.description && (
            <div className="mt-2">
              <Text type="secondary">{assistant.description}</Text>
            </div>
          )}
        </div>

        {/* Instructions */}
        <div>
          <Title level={5}>Instructions</Title>
          <Card size="sm" className="bg-gray-50">
            <Text className="text-sm whitespace-pre-wrap">
              {assistant.instructions}
            </Text>
          </Card>
        </div>

        {/* Dependencies — v2 Phase 7 replaces use_cases /
            example_prompts / recommended_models / recommended_mcp_servers
            with a single informational dependencies[] list. */}
        {assistant.dependencies && assistant.dependencies.length > 0 && (
          <div>
            <Title level={5}>Works best with</Title>
            <Flex wrap className="gap-1">
              {assistant.dependencies.map(dep => {
                const leaf = dep.name.split('/').slice(-1)[0]
                return (
                  <Tag
                    key={`${dep.kind}-${dep.name}`}
                    tone={dep.kind === 'model' ? 'success' : 'info'}
                  >
                    {leaf} {dep.versionRange}
                  </Tag>
                )
              })}
            </Flex>
          </div>
        )}

        {/* Assistant Details */}
        <div>
          <Title level={5}>Details</Title>
          <Flex direction="column" className="gap-2">
            {assistant.author && (
              <Flex justify="between">
                <Text type="secondary">Author:</Text>
                <Text>{assistant.author}</Text>
              </Flex>
            )}
          </Flex>
        </div>

        {/* Tags */}
        {assistant.tags && assistant.tags.length > 0 && (
          <div>
            <Title level={5}>Tags</Title>
            <Flex wrap className="gap-1">
              {assistant.tags.map(tag => (
                <Tag key={tag}>
                  {tag}
                </Tag>
              ))}
            </Flex>
          </div>
        )}

        {/* Parameters */}
        {assistant.parameters &&
          Object.keys(assistant.parameters).length > 0 && (
            <div>
              <Title level={5}>Parameters</Title>
              <Card size="sm">
                <pre className="text-xs overflow-auto m-0">
                  {JSON.stringify(assistant.parameters, null, 2)}
                </pre>
              </Card>
            </div>
          )}
      </Flex>
    </Sheet>
  )
}
