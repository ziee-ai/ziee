import {
  Card,
  Divider,
  Flex,
  Form,
  InputNumber,
  Select,
  Switch,
  Typography,
} from 'antd'
import { useEffect, useMemo } from 'react'

const { Text } = Typography

// Quantization options based on mistral.rs ISQ documentation
const QUANTIZATION_OPTIONS = {
  // AFQ Types (Metal only)
  AFQ: [
    { value: 'AFQ2', label: 'AFQ2', deviceTypes: ['metal'] },
    { value: 'AFQ3', label: 'AFQ3', deviceTypes: ['metal'] },
    { value: 'AFQ4', label: 'AFQ4', deviceTypes: ['metal'] },
    { value: 'AFQ6', label: 'AFQ6', deviceTypes: ['metal'] },
    { value: 'AFQ8', label: 'AFQ8', deviceTypes: ['metal'] },
  ],
  // Q-Types (all devices, with CUDA restrictions)
  Q_TYPES: [
    { value: 'Q4_0', label: 'Q4_0', deviceTypes: ['cpu', 'cuda', 'metal'] },
    { value: 'Q4_1', label: 'Q4_1', deviceTypes: ['cpu', 'cuda', 'metal'] },
    { value: 'Q5_0', label: 'Q5_0', deviceTypes: ['cpu', 'cuda', 'metal'] },
    { value: 'Q5_1', label: 'Q5_1', deviceTypes: ['cpu', 'cuda', 'metal'] },
    { value: 'Q8_0', label: 'Q8_0', deviceTypes: ['cpu', 'cuda', 'metal'] },
    { value: 'Q8_1', label: 'Q8_1', deviceTypes: ['cpu', 'metal'] }, // not on CUDA
  ],
  // K-Types (all devices, with CUDA restrictions)
  K_TYPES: [
    { value: 'Q2K', label: 'Q2K', deviceTypes: ['cpu', 'cuda', 'metal'] },
    { value: 'Q3K', label: 'Q3K', deviceTypes: ['cpu', 'cuda', 'metal'] },
    { value: 'Q4K', label: 'Q4K', deviceTypes: ['cpu', 'cuda', 'metal'] },
    { value: 'Q5K', label: 'Q5K', deviceTypes: ['cpu', 'cuda', 'metal'] },
    { value: 'Q6K', label: 'Q6K', deviceTypes: ['cpu', 'cuda', 'metal'] },
    { value: 'Q8K', label: 'Q8K', deviceTypes: ['cpu', 'metal'] }, // not on CUDA
  ],
  // Other Types
  OTHER: [
    { value: 'HQQ4', label: 'HQQ4', deviceTypes: ['cpu', 'cuda', 'metal'] },
    { value: 'HQQ8', label: 'HQQ8', deviceTypes: ['cpu', 'cuda', 'metal'] },
    { value: 'FP8', label: 'FP8', deviceTypes: ['cpu', 'cuda', 'metal'] },
  ],
}

// Flatten all quantization options
const ALL_QUANTIZATION_OPTIONS = [
  ...QUANTIZATION_OPTIONS.AFQ,
  ...QUANTIZATION_OPTIONS.Q_TYPES,
  ...QUANTIZATION_OPTIONS.K_TYPES,
  ...QUANTIZATION_OPTIONS.OTHER,
]

export function MistralRsLlmModelSettingsSection() {
  const form = Form.useFormInstance()

  // Watch for device type changes
  const selectedDeviceType =
    Form.useWatch(['engine_settings', 'mistralrs', 'device_type'], form) ||
    'cpu'
  const currentQuantization = Form.useWatch(
    ['engine_settings', 'mistralrs', 'in_situ_quant'],
    form,
  )

  // Filter quantization options based on device type
  const availableQuantizationOptions = useMemo(() => {
    return ALL_QUANTIZATION_OPTIONS.filter(option =>
      option.deviceTypes.includes(selectedDeviceType),
    ).map(option => ({
      value: option.value,
      label: option.label,
    }))
  }, [selectedDeviceType])

  // Clear incompatible quantization selection when device type changes
  useEffect(() => {
    if (currentQuantization) {
      const isCurrentQuantizationCompatible = ALL_QUANTIZATION_OPTIONS.find(
        option => option.value === currentQuantization,
      )?.deviceTypes.includes(selectedDeviceType)

      if (!isCurrentQuantizationCompatible) {
        form.setFieldValue(
          ['engine_settings', 'mistralrs', 'in_situ_quant'],
          undefined,
        )
      }
    }
  }, [selectedDeviceType, currentQuantization, form])

  const getFieldName = (field: string) => [
    'engine_settings',
    'mistralrs',
    field,
  ]

  const ResponsiveConfigItem = ({
    title,
    description,
    children,
  }: {
    title: string
    description: string
    children: React.ReactNode
  }) => (
    <Flex justify="space-between">
      <div>
        <Text strong>{title}</Text>
        <div>
          <Text type="secondary">{description}</Text>
        </div>
      </div>
      {children}
    </Flex>
  )

  return (
    <Flex vertical className="gap-4 w-full">
      {/* Sequence & Memory Management */}
      <Card title="Sequence & Memory Management">
        <Flex vertical className="gap-2 w-full">
          <ResponsiveConfigItem
            title="Max Sequences"
            description="Maximum running sequences at any time (default: 16)"
          >
            <Form.Item name={getFieldName('max_seqs')}>
              <InputNumber
                min={1}
                max={1024}
                placeholder="16"
                style={{ width: '100%' }}
              />
            </Form.Item>
          </ResponsiveConfigItem>

          <Divider style={{ margin: 0 }} />

          <ResponsiveConfigItem
            title="Max Sequence Length"
            description="Maximum prompt sequence length to expect for this model (default: 4096)"
          >
            <Form.Item name={getFieldName('max_seq_len')}>
              <InputNumber
                min={512}
                max={131072}
                placeholder="4096"
                style={{ width: '100%' }}
              />
            </Form.Item>
          </ResponsiveConfigItem>

          <Divider style={{ margin: 0 }} />

          <ResponsiveConfigItem
            title="No KV Cache"
            description="Use no KV cache"
          >
            <Form.Item
              name={getFieldName('no_kv_cache')}
              valuePropName="checked"
              style={{ margin: 0 }}
            >
              <Switch />
            </Form.Item>
          </ResponsiveConfigItem>

          <Divider style={{ margin: 0 }} />

          <ResponsiveConfigItem
            title="Truncate Sequence"
            description="If a sequence is larger than the maximum model length, truncate the number of tokens such that the sequence will fit at most the maximum length"
          >
            <Form.Item
              name={getFieldName('truncate_sequence')}
              valuePropName="checked"
              style={{ margin: 0 }}
            >
              <Switch />
            </Form.Item>
          </ResponsiveConfigItem>
        </Flex>
      </Card>

      {/* PagedAttention Configuration */}
      <Card title="PagedAttention Configuration">
        <Flex vertical className="gap-2 w-full">
          <ResponsiveConfigItem
            title="PagedAttention GPU Memory (MB)"
            description="GPU memory to allocate for KV cache with PagedAttention in MBs"
          >
            <Form.Item name={getFieldName('paged_attn_gpu_mem')}>
              <InputNumber
                min={128}
                max={65536}
                placeholder="Auto"
                style={{ width: '100%' }}
              />
            </Form.Item>
          </ResponsiveConfigItem>

          <Divider style={{ margin: 0 }} />

          <ResponsiveConfigItem
            title="PagedAttention GPU Memory Usage"
            description="Percentage of GPU memory to utilize after allocation of KV cache with PagedAttention, from 0 to 1 (default: 0.9 on CUDA)"
          >
            <Form.Item name={getFieldName('paged_attn_gpu_mem_usage')}>
              <InputNumber
                min={0.1}
                max={1.0}
                step={0.1}
                placeholder="0.9"
                style={{ width: '100%' }}
              />
            </Form.Item>
          </ResponsiveConfigItem>

          <Divider style={{ margin: 0 }} />

          <ResponsiveConfigItem
            title="PagedAttention Context Length"
            description="Total context length to allocate the KV cache for (total number of tokens which the KV cache can hold)"
          >
            <Form.Item name={getFieldName('paged_ctxt_len')}>
              <InputNumber
                min={512}
                max={131072}
                placeholder="Auto"
                style={{ width: '100%' }}
              />
            </Form.Item>
          </ResponsiveConfigItem>

          <Divider style={{ margin: 0 }} />

          <ResponsiveConfigItem
            title="PagedAttention Block Size"
            description="Block size (number of tokens per block) for PagedAttention (default: 32 on CUDA)"
          >
            <Form.Item name={getFieldName('paged_attn_block_size')}>
              <InputNumber
                min={1}
                max={512}
                placeholder="32"
                style={{ width: '100%' }}
              />
            </Form.Item>
          </ResponsiveConfigItem>

          <Divider style={{ margin: 0 }} />

          <ResponsiveConfigItem
            title="Disable PagedAttention"
            description="Disable PagedAttention on CUDA (PagedAttention is automatically activated on CUDA but not on Metal)"
          >
            <Form.Item
              name={getFieldName('no_paged_attn')}
              valuePropName="checked"
              style={{ margin: 0 }}
            >
              <Switch />
            </Form.Item>
          </ResponsiveConfigItem>

          <Divider style={{ margin: 0 }} />

          <ResponsiveConfigItem
            title="Enable PagedAttention on Metal"
            description="Enable PagedAttention on Metal (PagedAttention is automatically activated on CUDA but not on Metal)"
          >
            <Form.Item
              name={getFieldName('paged_attn')}
              valuePropName="checked"
              style={{ margin: 0 }}
            >
              <Switch />
            </Form.Item>
          </ResponsiveConfigItem>
        </Flex>
      </Card>

      {/* Performance Optimization */}
      <Card title="Performance Optimization">
        <Flex vertical className="gap-2 w-full">
          <ResponsiveConfigItem
            title="Prefix Cache Count"
            description="Number of prefix caches to hold on the device. Other caches are evicted to the CPU based on a LRU strategy (default: 16)"
          >
            <Form.Item name={getFieldName('prefix_cache_n')}>
              <InputNumber
                min={1}
                max={128}
                placeholder="16"
                style={{ width: '100%' }}
              />
            </Form.Item>
          </ResponsiveConfigItem>

          <Divider style={{ margin: 0 }} />

          <ResponsiveConfigItem
            title="Prompt Chunk Size"
            description="Number of tokens to batch the prompt step into. This can help with OOM errors when in the prompt step, but reduces performance"
          >
            <Form.Item name={getFieldName('prompt_chunksize')}>
              <InputNumber
                min={1}
                max={8192}
                placeholder="Auto"
                style={{ width: '100%' }}
              />
            </Form.Item>
          </ResponsiveConfigItem>
        </Flex>
      </Card>

      {/* Model Configuration */}
      <Card title="Model Configuration">
        <Flex vertical className="gap-2 w-full">
          <ResponsiveConfigItem
            title="Data Type"
            description="Model data type (default: auto)"
          >
            <Form.Item name={getFieldName('dtype')}>
              <Select
                placeholder="auto"
                style={{ width: '100%' }}
                allowClear
                options={[
                  { value: 'auto', label: 'Auto' },
                  { value: 'f16', label: 'Float16' },
                  { value: 'f32', label: 'Float32' },
                  { value: 'bf16', label: 'BFloat16' },
                ]}
              />
            </Form.Item>
          </ResponsiveConfigItem>

          <Divider style={{ margin: 0 }} />

          <ResponsiveConfigItem
            title="In-Situ Quantization"
            description={`In-situ quantization to apply (${selectedDeviceType.toUpperCase()})`}
          >
            <Form.Item name={getFieldName('in_situ_quant')}>
              <Select
                placeholder="None"
                style={{ width: '100%' }}
                allowClear
                options={availableQuantizationOptions}
              />
            </Form.Item>
          </ResponsiveConfigItem>

          <Divider style={{ margin: 0 }} />

          <ResponsiveConfigItem
            title="Random Seed"
            description="Integer seed to ensure reproducible random number generation"
          >
            <Form.Item name={getFieldName('seed')}>
              <InputNumber
                min={0}
                max={4294967295}
                placeholder="Random"
                style={{ width: '100%' }}
              />
            </Form.Item>
          </ResponsiveConfigItem>
        </Flex>
      </Card>

      {/* Vision Model Settings */}
      <Card title="Vision Model Settings">
        <Flex vertical className="gap-2 w-full">
          <ResponsiveConfigItem
            title="Max Edge Length (Vision)"
            description="Automatically resize and pad images to this maximum edge length. Aspect ratio is preserved (vision models only)"
          >
            <Form.Item name={getFieldName('max_edge')}>
              <InputNumber
                min={224}
                max={2048}
                placeholder="Auto"
                style={{ width: '100%' }}
              />
            </Form.Item>
          </ResponsiveConfigItem>

          <Divider style={{ margin: 0 }} />

          <ResponsiveConfigItem
            title="Max Number of Images (Vision)"
            description="Maximum prompt number of images to expect for this model (vision models only)"
          >
            <Form.Item name={getFieldName('max_num_images')}>
              <InputNumber
                min={1}
                max={32}
                placeholder="Auto"
                style={{ width: '100%' }}
              />
            </Form.Item>
          </ResponsiveConfigItem>

          <Divider style={{ margin: 0 }} />

          <ResponsiveConfigItem
            title="Max Image Length (Vision)"
            description="Maximum expected image size will have this edge length on both edges (vision models only)"
          >
            <Form.Item name={getFieldName('max_image_length')}>
              <InputNumber
                min={224}
                max={2048}
                placeholder="Auto"
                style={{ width: '100%' }}
              />
            </Form.Item>
          </ResponsiveConfigItem>
        </Flex>
      </Card>
    </Flex>
  )
}
