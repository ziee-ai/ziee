import type { Assistant } from '@/api-client/types'
import { EventBus } from '@ziee/framework/stores'

export const emitAssistantCreated = async (assistant: Assistant) => {
  await EventBus.emit({
    type: 'assistant.created',
    data: { assistant },
  })
}

export const emitAssistantUpdated = async (assistant: Assistant) => {
  await EventBus.emit({
    type: 'assistant.updated',
    data: { assistant },
  })
}

export const emitAssistantDeleted = async (assistantId: string) => {
  await EventBus.emit({
    type: 'assistant.deleted',
    data: { assistantId },
  })
}

export const emitAssistantTemplateCreated = async (template: Assistant) => {
  await EventBus.emit({
    type: 'assistant_template.created',
    data: { template },
  })
}

export const emitAssistantTemplateUpdated = async (template: Assistant) => {
  await EventBus.emit({
    type: 'assistant_template.updated',
    data: { template },
  })
}

export const emitAssistantTemplateDeleted = async (templateId: string) => {
  await EventBus.emit({
    type: 'assistant_template.deleted',
    data: { templateId },
  })
}
