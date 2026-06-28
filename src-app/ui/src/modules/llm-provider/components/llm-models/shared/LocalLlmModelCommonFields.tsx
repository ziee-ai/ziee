import { FormField, Select } from '@/components/ui'
import { LOCAL_FILE_TYPE_OPTIONS } from '@/modules/llm-provider/constants/localLlmModelTypes'
import { LlmModelParametersSection } from '@/modules/llm-provider/components/llm-models/shared/LlmModelParametersSection'
import { LOCAL_MODEL_FIELDS } from '@/modules/llm-provider/constants/llmModelParameters'

const ENGINE_OPTIONS = [
  {
    value: 'mistralrs',
    label: 'MistralRs',
    description: 'High-performance inference engine with advanced features',
  },
  {
    value: 'llamacpp',
    label: 'LlamaCpp',
    description: 'Coming soon - GGML-based inference engine',
  },
]

export function LocalLlmModelCommonFields() {
  return (
    <>
      <LlmModelParametersSection parameters={LOCAL_MODEL_FIELDS} />

      <FormField
        name="engine_type"
        label="Engine Type"
        required
      >
        <Select
          placeholder="Select Engine Type"
          options={ENGINE_OPTIONS.map(option => ({
            value: option.value,
            label: option.label,
            disabled: option.value === 'llamacpp', // Disable LlamaCpp for now
          }))}
        />
      </FormField>

      <FormField
        name="file_format"
        label="File Format"
        required
      >
        <Select
          placeholder="Select file format"
          options={LOCAL_FILE_TYPE_OPTIONS.map(option => ({
            value: option.value,
            label: option.label,
            description: option.description,
          }))}
        />
      </FormField>
    </>
  )
}
