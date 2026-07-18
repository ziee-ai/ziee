import { FormField, Select, Button, Tooltip } from '@ziee/kit'
import { CircleAlert } from 'lucide-react'
import { useNavigate } from 'react-router-dom'

interface ModelSelectorProps {
  isBreaking: boolean
  isDisabled: boolean
  availableModels: Array<{
    label: string
    options: Array<{ label: string; value: string; description?: string }>
  }>
}

export function ModelSelector({
  isBreaking,
  isDisabled,
  availableModels,
}: ModelSelectorProps) {
  const navigate = useNavigate()
  const hasModels = availableModels.some(g => g.options.length > 0)

  // Empty state: a bare Select would render an all-but-invisible trigger with an
  // empty dropdown and no guidance. Show an explicit, actionable affordance
  // instead — "No models" + a tooltip/CTA that routes to provider settings.
  if (!hasModels) {
    return (
      <FormField name="model" label="Model" className="mb-0 inline-block">
        <Tooltip content="No models available — add an LLM provider in Settings">
          {isBreaking ? (
            <Button
              variant="outline"
              size="icon"
              icon={<CircleAlert className="text-muted-foreground" />}
              onClick={() => navigate('/settings/llm-providers')}
              data-testid="chat-model-select-empty"
              aria-label="No models available — add an LLM provider"
              className="w-10"
            />
          ) : (
            <Button
              variant="outline"
              icon={<CircleAlert className="text-muted-foreground" />}
              onClick={() => navigate('/settings/llm-providers')}
              data-testid="chat-model-select-empty"
              aria-label="No models available — add an LLM provider"
              className="w-[140px] justify-start font-normal text-muted-foreground"
            >
              No models — Add
            </Button>
          )}
        </Tooltip>
      </FormField>
    )
  }

  return (
    <FormField
      name="model"
      label="Model"
      className={`mb-0 inline-block`}
    >
      <Select
        data-testid="chat-model-select"
        popupMatchSelectWidth={false}
        placeholder="Model"
        disabled={isDisabled}
        options={availableModels}
        className={isBreaking ? 'w-10' : 'w-[120px]'}
        labelRender={isBreaking ? () => '' : undefined}
      />
    </FormField>
  )
}
