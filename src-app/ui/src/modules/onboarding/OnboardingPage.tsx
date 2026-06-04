import {
  useState,
  useMemo,
  useCallback,
  useEffect,
  useRef,
  Suspense,
} from 'react'
import { useNavigate, useSearchParams } from 'react-router-dom'
import {
  Typography,
  Steps,
  Button,
  Alert,
  Spin,
  theme,
} from 'antd'
import { CheckCircleOutlined, BookOutlined, ArrowLeftOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'
import type { OnboardingSlot } from './types/OnboardingSlot'
import type { OnboardingStepProps } from './types/onboarding'

const { Title, Text } = Typography

export default function OnboardingPage() {
  const { token } = theme.useToken()
  const navigate = useNavigate()
  const [searchParams] = useSearchParams()

  const nextEnabled = Stores.Onboarding.nextEnabled
  const nextLoading = Stores.Onboarding.nextLoading
  const nextError = Stores.Onboarding.nextError
  const completedGuideIds = Stores.Onboarding.completedGuideIds
  const completedStepIds = Stores.Onboarding.completedStepIds
  const loaded = Stores.Onboarding.loaded
  const slots = Stores.ModuleSystem.slots

  // Holds the async action registered by the current step (not in store — functions don't go in Zustand/immer)
  const beforeNextRef = useRef<(() => Promise<void>) | null>(null)

  useEffect(() => {
    return () => {
      Stores.Onboarding.reset()
      Stores.ApiKeysStep.reset()
      Stores.McpServersStep.reset()
    }
  }, [])

  const guides = useMemo(
    () => ((slots.get('onboarding') as OnboardingSlot[]) || []).sort((a, b) => a.order - b.order),
    [slots],
  )

  const getInitialStepIndex = useCallback((g: OnboardingSlot): number => {
    const idx = g.steps.findIndex(s => !completedStepIds.includes(`${g.id}/${s.id}`))
    return idx === -1 ? 0 : idx
  }, [completedStepIds])

  const guideId = searchParams.get('id') || guides[0]?.id
  const [activeGuideId, setActiveGuideId] = useState(guideId)
  const guide = guides.find(g => g.id === activeGuideId) ?? guides[0]

  const baseStep = useMemo(
    () => (loaded && guide ? getInitialStepIndex(guide) : 0),
    [loaded, guide, getInitialStepIndex],
  )

  const [manualStep, setManualStep] = useState<number | null>(null)
  const currentStepIndex = manualStep ?? baseStep

  const currentStep = guide?.steps[currentStepIndex]

  // Reset navigation state and before-next action on every step change
  useEffect(() => {
    Stores.Onboarding.setReady(currentStep?.skippable !== false)
    Stores.Onboarding.setNextError(null)
    beforeNextRef.current = null
  }, [currentStepIndex, activeGuideId])

  const handleGlobalNext = useCallback(async () => {
    if (!guide) return
    Stores.Onboarding.setNextLoading(true)
    Stores.Onboarding.setNextError(null)
    try {
      await beforeNextRef.current?.()
      await Stores.Onboarding.completeStep(guide.id, guide.steps[currentStepIndex].id)
      if (currentStepIndex === guide.steps.length - 1) {
        await Stores.Onboarding.completeGuide(guide.id)
        Stores.Onboarding.reset()
        Stores.ApiKeysStep.reset()
        Stores.McpServersStep.reset()
        navigate('/chat', { replace: true })
      } else {
        setManualStep(currentStepIndex + 1)
      }
    } catch (err: any) {
      Stores.Onboarding.setNextError(err.message || 'Something went wrong')
    } finally {
      Stores.Onboarding.setNextLoading(false)
    }
  }, [guide, currentStepIndex, navigate])

  const handleSelectGuide = (g: OnboardingSlot) => {
    setActiveGuideId(g.id)
    setManualStep(null)
  }

  if (!guide) {
    return <div className="p-8"><Text>No guides available.</Text></div>
  }

  const StepComponent = currentStep?.component
  const isLastStep = currentStepIndex === guide.steps.length - 1

  const stepProps: OnboardingStepProps = {
    registerBeforeNext: (fn) => { beforeNextRef.current = fn },
  }

  return (
    <div
      className="flex h-screen overflow-hidden"
      style={{ backgroundColor: token.colorBgLayout }}
    >
      {/* Left pane: guide list */}
      <div
        className="w-64 flex-shrink-0 border-r overflow-y-auto p-4 flex flex-col gap-2"
        style={{
          borderColor: token.colorBorderSecondary,
          backgroundColor: token.colorBgContainer,
        }}
      >
        <div className="flex items-center gap-2 mb-2">
          <BookOutlined className="text-lg" />
          <Text strong>Onboarding</Text>
        </div>
        <Button
          type="text"
          size="small"
          icon={<ArrowLeftOutlined />}
          onClick={() => navigate('/chat')}
          className="!px-0 mb-3"
        >
          Back to Chat
        </Button>
        {guides.map(g => {
          const isCompleted = completedGuideIds.includes(g.id)
          const isActive = g.id === activeGuideId
          return (
            <div
              key={g.id}
              className="p-3 rounded-lg cursor-pointer transition-colors"
              style={{
                backgroundColor: isActive ? token.colorPrimaryBg : undefined,
                border: `1px solid ${isActive ? token.colorPrimary : token.colorBorderSecondary}`,
              }}
              onClick={() => handleSelectGuide(g)}
            >
              <div className="flex items-center justify-between">
                <Text strong className={isActive ? 'text-primary' : ''}>
                  {g.title}
                </Text>
                {isCompleted && (
                  <CheckCircleOutlined className="text-green-500" />
                )}
              </div>
              <Text type="secondary" className="text-xs block mt-1">
                {g.description}
              </Text>
            </div>
          )
        })}
      </div>

      {/* Right pane: step viewer */}
      <div className="flex-1 flex flex-col overflow-hidden">
        {/* Header with steps */}
        <div
          className="p-6 border-b"
          style={{
            borderColor: token.colorBorderSecondary,
            backgroundColor: token.colorBgContainer,
          }}
        >
          <Title level={4} className="!mb-3">
            {guide.title}
          </Title>
          <Steps
            current={currentStepIndex}
            items={guide.steps.map(s => ({ title: s.title }))}
            size="small"
          />
        </div>

        {/* Step content */}
        <div className="flex-1 overflow-y-auto p-6">
          {nextError && (
            <Alert
              type="error"
              title={nextError}
              showIcon
              closable={{ onClose: () => Stores.Onboarding.setNextError(null) }}
              className="mb-4"
            />
          )}
          {StepComponent && (
            <Suspense fallback={<Spin className="flex justify-center mt-8" />}>
              <StepComponent {...stepProps} />
            </Suspense>
          )}
        </div>

        {/* Footer navigation */}
        <div
          className="p-4 border-t flex justify-between items-center"
          style={{
            borderColor: token.colorBorderSecondary,
            backgroundColor: token.colorBgContainer,
          }}
        >
          <Button
            disabled={currentStepIndex === 0}
            onClick={() => setManualStep(currentStepIndex - 1)}
          >
            Back
          </Button>
          <Button
            type="primary"
            disabled={!nextEnabled}
            loading={nextLoading}
            onClick={handleGlobalNext}
          >
            {isLastStep ? 'Start Chatting' : 'Next'}
          </Button>
        </div>
      </div>
    </div>
  )
}
