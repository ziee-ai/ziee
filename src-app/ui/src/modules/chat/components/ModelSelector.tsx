import { FormField, Select } from '@/components/ui'

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
  return (
    <FormField
      name="model"
      label="Model"
      className={`mb-0 inline-block`}
    >
      <Select
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
