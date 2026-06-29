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
  Button,
  Alert,
  Spin,
  Text,
  Title,
  Progress,
} from '@/components/ui'
import { CircleCheck, Book, ArrowLeft } from 'lucide-react'
import { Stores } from '@/core/stores'
import type { OnboardingSlot } from './types/OnboardingSlot'
import type { OnboardingStepProps } from './types/onboarding'

export default function OnboardingPage() {
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
    return (
      <div
        data-testid="onboarding-empty-state"
        className="flex flex-col items-center justify-center h-screen gap-4 p-8 bg-background text-center"
      >
        <div className="flex h-16 w-16 items-center justify-center rounded-full bg-muted">
          <Book className="h-8 w-8 text-muted-foreground" />
        </div>
        <Title level={4} className="!mb-0">
          No onboarding guides available
        </Title>
        <Text type="secondary" className="max-w-md">
          Onboarding guides walk you through first-time setup — connecting a model
          provider, configuring tools, and personalizing your workspace. None have
          been configured for this deployment yet.
        </Text>
        <Text type="secondary" className="max-w-md text-sm">
          Check back after your administrator finishes setup, or jump straight into
          chat to get started.
        </Text>
        <Button
          data-testid="onboarding-no-guides-go-to-chat"
          variant="default"
          onClick={() => navigate('/chat')}
        >
          Go to Chat
        </Button>
      </div>
    )
  }

  const StepComponent = currentStep?.component
  const isLastStep = currentStepIndex === guide.steps.length - 1

  const stepProps: OnboardingStepProps = {
    registerBeforeNext: (fn) => { beforeNextRef.current = fn },
  }

  return (
    <div className="flex flex-col md:flex-row h-screen overflow-hidden bg-background">
      {/* Left pane: guide list. On small screens it collapses into a
          height-capped, scrollable strip above the step viewer (the
          two-pane row stacks vertically below the md breakpoint). */}
      <div className="w-full md:w-64 flex-shrink-0 max-h-44 md:max-h-none border-b md:border-r overflow-y-auto p-4 flex flex-col gap-2 border-border bg-card">
        <div className="flex items-center gap-2 mb-2">
          <Book className="text-lg" />
          <Text strong>Onboarding</Text>
        </div>
        <Button
          data-testid="onboarding-page-back-to-chat-button"
          variant="ghost"
          size="sm"
          icon={<ArrowLeft />}
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
              data-testid={`onboarding-guide-card-${g.id}`}
              className={`p-3 rounded-lg cursor-pointer transition-colors ${isActive ? 'bg-accent border border-primary' : 'border border-border'}`}
              onClick={() => handleSelectGuide(g)}
            >
              <div className="flex items-center justify-between">
                <Text strong className={isActive ? 'text-primary' : ''}>
                  {g.title}
                </Text>
                {isCompleted && (
                  <CircleCheck className="text-success" />
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
        <div className="p-4 md:p-6 border-b border-border bg-card">
          <Title level={4} className="!mb-3">
            {guide.title}
          </Title>
          <Progress
            data-testid="onboarding-page-step-progress"
            value={Math.round(((currentStepIndex + 1) / guide.steps.length) * 100)}
            showInfo={false}
            size="sm"
            aria-label={`Step ${currentStepIndex + 1} of ${guide.steps.length}`}
          />
        </div>

        {/* Step content */}
        <div className="flex-1 overflow-y-auto p-4 md:p-6">
          {nextError && (
            <Alert
              data-testid="onboarding-page-next-error-alert"
              tone="error"
              title={nextError}
              onClose={() => Stores.Onboarding.setNextError(null)}
              closeLabel="Close"
              className="mb-4"
            />
          )}
          {StepComponent && (
            <div data-testid={`onboarding-step-${currentStep.id}`}>
              <Suspense
                fallback={
                  <Spin
                    data-testid="onboarding-step-loading"
                    className="flex justify-center mt-8"
                    label="Loading step"
                  />
                }
              >
                <StepComponent {...stepProps} />
              </Suspense>
            </div>
          )}
        </div>

        {/* Footer navigation */}
        <div className="p-4 border-t flex justify-between items-center border-border bg-card">
          <Button
            data-testid="onboarding-page-back-button"
            disabled={currentStepIndex === 0}
            onClick={() => setManualStep(currentStepIndex - 1)}
          >
            Back
          </Button>
          <Button
            data-testid="onboarding-page-next-button"
            variant="default"
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
