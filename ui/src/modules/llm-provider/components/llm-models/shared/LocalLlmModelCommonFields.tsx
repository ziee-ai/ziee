import { Form, Select } from 'antd'
import { LOCAL_FILE_TYPE_OPTIONS } from '../../../constants/localLlmModelTypes'
import { LlmModelParametersSection } from './LlmModelParametersSection'
import { LOCAL_MODEL_FIELDS } from '../../../constants/llmModelParameters'

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

      <Form.Item
        name="engine_type"
        label="Engine"
        rules={[
          {
            required: true,
            message: 'Please select an engine',
          },
        ]}
        initialValue="mistralrs"
      >
        <Select
          placeholder="Select Engine"
          options={ENGINE_OPTIONS.map(option => ({
            value: option.value,
            label: option.label,
            disabled: option.value === 'llamacpp', // Disable LlamaCpp for now
          }))}
        />
      </Form.Item>

      <Form.Item
        name="file_format"
        label="File Format"
        rules={[
          {
            required: true,
            message: 'Please select a file format',
          },
        ]}
      >
        <Select
          placeholder="Select file format"
          options={LOCAL_FILE_TYPE_OPTIONS.map(option => ({
            value: option.value,
            label: option.label,
            description: option.description,
          }))}
        />
      </Form.Item>
    </>
  )
}
