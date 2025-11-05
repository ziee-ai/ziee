export interface ParameterFieldConfig {
  name: string | string[]
  label: string
  help?: string
  placeholder?: string
  type: 'number' | 'text' | 'password' | 'textarea' | 'select' | 'string-array'
  min?: number
  max?: number
  step?: number
  required?: boolean
  options?: Array<{ value: string | number; label: string }>
  rules?: any[]
}

export const MODEL_PARAMETERS: ParameterFieldConfig[] = [
  {
    name: ['parameters', 'temperature'],
    label: 'Temperature',
    type: 'number',
    min: 0,
    max: 2,
    step: 0.1,
    placeholder: '0.7',
    help: 'Controls randomness in generation (0.0 = deterministic, 1.0 = very random)',
  },
  {
    name: ['parameters', 'top_p'],
    label: 'Top P',
    type: 'number',
    min: 0,
    max: 1,
    step: 0.05,
    placeholder: '0.9',
    help: 'Nucleus sampling - only consider tokens with cumulative probability up to this value',
  },
  {
    name: ['parameters', 'top_k'],
    label: 'Top K',
    type: 'number',
    min: 1,
    placeholder: '50',
    help: 'Only consider the top K most likely tokens',
  },
  {
    name: ['parameters', 'max_tokens'],
    label: 'Max Tokens',
    type: 'number',
    min: 1,
    max: 8192,
    placeholder: '512',
    help: 'Maximum number of tokens to generate',
  },
  {
    name: ['parameters', 'repeat_penalty'],
    label: 'Repeat Penalty',
    type: 'number',
    min: 0.1,
    max: 2,
    step: 0.1,
    placeholder: '1.1',
    help: 'Penalty for repeating tokens (1.0 = no penalty, >1.0 = discourage repetition)',
  },
  {
    name: ['parameters', 'repeat_last_n'],
    label: 'Repeat Last N',
    type: 'number',
    min: 0,
    placeholder: '64',
    help: 'Number of previous tokens to consider for repeat penalty',
  },
  {
    name: ['parameters', 'seed'],
    label: 'Seed',
    type: 'number',
    placeholder: 'Leave empty for random',
    help: 'Random seed for reproducible outputs. Use the same seed to get consistent results.',
  },
  {
    name: ['parameters', 'stop'],
    label: 'Stop Sequences',
    type: 'string-array',
    placeholder: 'Enter stop sequence',
    help: 'Stop generation when any of these sequences are encountered (max 4 sequences)',
  },
]

export const BASIC_MODEL_FIELDS: ParameterFieldConfig[] = [
  {
    name: 'name',
    label: 'Model ID',
    type: 'text',
    required: true,
    placeholder: 'e.g., llama-2-7b-chat',
    help: 'Unique identifier for this model',
  },
  {
    name: 'display_name',
    label: 'Display Name',
    type: 'text',
    required: true,
    placeholder: 'e.g., Llama 2 7B Chat',
    help: 'Human-readable name shown in the interface',
  },
  {
    name: 'description',
    label: 'Description',
    type: 'textarea',
    placeholder: 'Optional description of this model...',
  },
]

export const LOCAL_MODEL_FIELDS: ParameterFieldConfig[] = [
  {
    name: 'display_name',
    label: 'Display Name',
    type: 'text',
    required: true,
    placeholder: 'e.g., Llama 2 7B Chat',
    help: 'Human-readable name shown in the interface',
  },
  {
    name: 'description',
    label: 'Description',
    type: 'textarea',
    placeholder: 'Optional description of this model...',
  },
]

export const DEVICE_CONFIGURATION_FIELDS: ParameterFieldConfig[] = [
  {
    name: 'device_type',
    label: 'Device Type',
    type: 'select',
    required: true,
    placeholder: 'Select device type',
    help: 'The type of compute device to use for model inference',
  },
  {
    name: 'device_ids',
    label: 'Specific Devices',
    type: 'select',
    placeholder: 'Select specific devices (optional)',
    help: 'Select specific devices to use. Leave empty to use the default device.',
  },
]
