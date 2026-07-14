import { useEffect } from 'react'
import { Title, Paragraph } from '@ziee/kit'
import { Rocket } from 'lucide-react'
import type { OnboardingStepProps } from '@/modules/onboarding/types/onboarding'
import { Stores } from '@ziee/framework/stores'

export default function WelcomeStep({ registerBeforeNext }: OnboardingStepProps) {
  const user = Stores.Auth.user

  useEffect(() => {
    Stores.Onboarding.setReady(true)
    registerBeforeNext(null)
  }, [])

  return (
    <div className="max-w-lg">
      <div className="flex items-center gap-3 mb-4">
        <Rocket className="text-4xl text-primary" />
        <Title level={3} className="!mb-0">
          Welcome{user?.display_name ? `, ${user.display_name}` : ''}!
        </Title>
      </div>

      <Paragraph className="text-base">
        This quick setup will help you configure Ziee so it&apos;s ready
        to use right away. It only takes a few minutes.
      </Paragraph>

      <Paragraph type="secondary">
        You&apos;ll be able to:
      </Paragraph>

      <ul className="list-disc pl-6 space-y-1 mb-6 text-muted-foreground">
        <li>Connect your AI provider API keys</li>
        <li>Enable MCP servers to extend your AI&apos;s capabilities</li>
        <li>Start chatting with a fully configured setup</li>
      </ul>
    </div>
  )
}
