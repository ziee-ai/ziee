import { useEffect } from 'react'
import { Typography } from 'antd'
import { RocketOutlined } from '@ant-design/icons'
import type { OnboardingStepProps } from '@/modules/onboarding-screen/types/onboarding'
import { Stores } from '@/core/stores'

const { Title, Paragraph } = Typography

export default function WelcomeStep({ registerBeforeNext }: OnboardingStepProps) {
  const user = Stores.Auth.user

  useEffect(() => {
    Stores.OnboardingScreen.setReady(true)
    registerBeforeNext(null)
  }, [])

  return (
    <div className="max-w-lg">
      <div className="flex items-center gap-3 mb-4">
        <RocketOutlined className="text-4xl text-blue-500" />
        <Title level={3} className="!mb-0">
          Welcome{user?.display_name ? `, ${user.display_name}` : ''}!
        </Title>
      </div>

      <Paragraph className="text-base">
        This quick setup will help you configure Ziee Chat so it&apos;s ready
        to use right away. It only takes a few minutes.
      </Paragraph>

      <Paragraph type="secondary">
        You&apos;ll be able to:
      </Paragraph>

      <ul className="list-disc pl-6 space-y-1 mb-6 text-gray-600">
        <li>Connect your AI provider API keys</li>
        <li>Enable MCP servers to extend your AI&apos;s capabilities</li>
        <li>Start chatting with a fully configured setup</li>
      </ul>
    </div>
  )
}
