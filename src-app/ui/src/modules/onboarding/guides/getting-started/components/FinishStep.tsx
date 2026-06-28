import { useEffect } from 'react'
import { Typography } from 'antd'
import { CheckCircleOutlined, RocketOutlined } from '@ant-design/icons'
import type { OnboardingStepProps } from '@/modules/onboarding/types/onboarding'
import { Stores } from '@/core/stores'

const { Title, Paragraph, Text } = Typography

export default function FinishStep({ registerBeforeNext }: OnboardingStepProps) {
  const selectedMcpServerIds = Stores.McpServersStep.__state.selectedMcpServerIds
  const enteredApiKeys = Stores.ApiKeysStep.enteredApiKeys

  const apiKeysCount = Object.values(enteredApiKeys).filter(k => k.trim()).length
  const mcpCount = selectedMcpServerIds.length

  useEffect(() => {
    Stores.Onboarding.setReady(true)
    // Hub-server installations and system-server toggles were already applied
    // when the user clicked Next on the McpServersStep (if visited).
    // Re-apply here as a safety net for guides that skip the MCP step.
    registerBeforeNext(async () => {
      await Stores.McpServersStep.applyMcpServerChanges()
    })
  }, [])

  return (
    <div className="max-w-lg">
      <div className="flex items-center gap-3 mb-4">
        <RocketOutlined className="text-4xl text-green-500" />
        <Title level={3} className="!mb-0">
          You&apos;re all set!
        </Title>
      </div>

      <Paragraph>Here&apos;s a summary of what you configured:</Paragraph>

      <div className="space-y-2 mb-6">
        <div className="flex items-center gap-2">
          <CheckCircleOutlined className="text-green-500" />
          <Text>
            {apiKeysCount > 0
              ? `${apiKeysCount} API key${apiKeysCount > 1 ? 's' : ''} saved`
              : 'No API keys added (you can add them later in Settings)'}
          </Text>
        </div>
        <div className="flex items-center gap-2">
          <CheckCircleOutlined className="text-green-500" />
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
