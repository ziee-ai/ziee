import { useEffect } from 'react'
import { CircleCheck, Rocket } from 'lucide-react'
import type { OnboardingStepProps } from '@/modules/onboarding/types/onboarding'
import { Stores } from '@/core/stores'
import { Title, Paragraph, Text } from '@/components/ui'

export default function FinishStep({ registerBeforeNext }: OnboardingStepProps) {
  const selectedMcpServerIds = Stores.McpServersStep.selectedMcpServerIds
  const enteredApiKeys = Stores.ApiKeysStep.enteredApiKeys

  const apiKeysCount = Object.values(enteredApiKeys).filter(k => k.trim()).length
  const mcpCount = selectedMcpServerIds.length

  useEffect(() => {
    Stores.Onboarding.setReady(true)
    registerBeforeNext(async () => {
      await Stores.McpServersStep.installSelectedMcpServers()
    })
  }, [])

  return (
    <div className="max-w-lg">
      <div className="flex items-center gap-3 mb-4">
        <Rocket className="text-4xl text-primary" />
        <Title level={3} className="!mb-0">
          You&apos;re all set!
        </Title>
      </div>

      <Paragraph>Here&apos;s a summary of what you configured:</Paragraph>

      <div className="space-y-2 mb-6">
        <div className="flex items-center gap-2">
          <CircleCheck className="text-primary" />
          <Text>
            {apiKeysCount > 0
              ? `${apiKeysCount} API key${apiKeysCount > 1 ? 's' : ''} saved`
              : 'No API keys added (you can add them later in Settings)'}
          </Text>
        </div>
        <div className="flex items-center gap-2">
          <CircleCheck className="text-primary" />
          <Text>
            {mcpCount > 0
              ? `${mcpCount} MCP server${mcpCount > 1 ? 's' : ''} selected for installation`
              : 'No MCP servers selected (you can add them later)'}
          </Text>
        </div>
      </div>
    </div>
  )
}
