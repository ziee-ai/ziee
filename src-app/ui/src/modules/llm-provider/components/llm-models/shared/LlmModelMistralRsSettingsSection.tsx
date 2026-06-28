import * as React from 'react'
import {
  Card,
  Separator,
  Flex,
  FormField,
  InputNumber,
  Select,
  Switch,
  Text,
  useWatch,
  useFormContext,
} from '@/components/ui'
import { useEffect, useMemo } from 'react'

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

export function LlmModelMistralRsSettingsSection() {
  const form = useFormContext()

  // Watch for device type changes
  const selectedDeviceType =
    useWatch({ name: 'engine_settings.mistralrs.device_type' }) || 'cpu'
  const currentQuantization = useWatch({
    name: 'engine_settings.mistralrs.in_situ_quant',
  })

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
        form.setValue(
          'engine_settings.mistralrs.in_situ_quant',
          undefined,
        )
      }
    }
  }, [selectedDeviceType, currentQuantization, form])

  const getFieldName = (field: string) =>
    `engine_settings.mistralrs.${field}`

  const ResponsiveConfigItem = ({
    title,
    description,
    children,
  }: {
    title: string
    description: string
    children: React.ReactNode
  }) => (
    <Flex justify="between">
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
      {/* Device */}
      <Card title="Device">
        <Flex vertical className="gap-2 w-full">
          <ResponsiveConfigItem
            title="Device Type"
            description="Hardware backend to run the model on. Leave empty to auto-select; CPU runs inference on the CPU."
          >
            <FormField name={getFieldName('device_type')} aria-label="Device Type" className="m-0">
              <Select
                placeholder="Auto"
                className="w-[120px]"
                options={[
                  { value: 'cpu', label: 'CPU' },
                  { value: 'cuda', label: 'CUDA' },
                  { value: 'metal', label: 'Metal' },
                  { value: 'rocm', label: 'ROCm' },
                  { value: 'vulkan', label: 'Vulkan' },
                ]}
              />
            </FormField>
          </ResponsiveConfigItem>
        </Flex>
      </Card>

      {/* Sequence & Memory Management */}
      <Card title="Sequence & Memory Management">
        <Flex vertical className="gap-2 w-full">
          <ResponsiveConfigItem
            title="Max Sequences"
            description="Maximum running sequences at any time (default: 16)"
          >
            <FormField name={getFieldName('max_seqs')} aria-label="Max Sequences">
              <InputNumber
                min={1}
                max={1024}
                placeholder="16"
                className="w-full"
              />
            </FormField>
          </ResponsiveConfigItem>

          <Separator />

          <ResponsiveConfigItem
            title="No KV Cache"
            description="Use no KV cache"
          >
            <FormField
              name={getFieldName('no_kv_cache')} aria-label="No KV Cache"
              valuePropName="checked"
              className="m-0"
            >
              <Switch />
            </FormField>
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
            <FormField name={getFieldName('paged_attn_gpu_mem')} aria-label="PagedAttention GPU Memory (MB)">
              <InputNumber
                min={128}
                max={65536}
                placeholder="Auto"
                className="w-full"
              />
            </FormField>
          </ResponsiveConfigItem>

          <Separator />

          <ResponsiveConfigItem
            title="PagedAttention GPU Memory Usage"
            description="Percentage of GPU memory to utilize after allocation of KV cache with PagedAttention, from 0 to 1 (default: 0.9 on CUDA)"
          >
            <FormField name={getFieldName('paged_attn_gpu_mem_usage')} aria-label="PagedAttention GPU Memory Usage">
              <InputNumber
                min={0.1}
                max={1.0}
                step={0.1}
                placeholder="0.9"
                className="w-full"
              />
            </FormField>
          </ResponsiveConfigItem>

          <Separator />

          <ResponsiveConfigItem
            title="PagedAttention Context Length"
            description="Total context length to allocate the KV cache for (total number of tokens which the KV cache can hold)"
          >
            <FormField name={getFieldName('paged_ctxt_len')} aria-label="PagedAttention Context Length">
              <InputNumber
                min={512}
                max={131072}
                placeholder="Auto"
                className="w-full"
              />
            </FormField>
          </ResponsiveConfigItem>

          <Separator />

          <ResponsiveConfigItem
            title="PagedAttention Block Size"
            description="Block size (number of tokens per block) for PagedAttention (default: 32 on CUDA)"
          >
            <FormField name={getFieldName('paged_attn_block_size')} aria-label="PagedAttention Block Size">
              <InputNumber
                min={1}
                max={512}
                placeholder="32"
                className="w-full"
              />
            </FormField>
          </ResponsiveConfigItem>

          <Separator />

          <ResponsiveConfigItem
            title="Disable PagedAttention"
            description="Disable PagedAttention on CUDA (PagedAttention is automatically activated on CUDA but not on Metal)"
          >
            <FormField
              name={getFieldName('no_paged_attn')} aria-label="Disable PagedAttention"
              valuePropName="checked"
              className="m-0"
            >
              <Switch />
            </FormField>
          </ResponsiveConfigItem>

          <Separator />

          <ResponsiveConfigItem
            title="Enable PagedAttention on Metal"
            description="Enable PagedAttention on Metal (PagedAttention is automatically activated on CUDA but not on Metal)"
          >
            <FormField
              name={getFieldName('paged_attn')} aria-label="Enable PagedAttention on Metal"
              valuePropName="checked"
              className="m-0"
            >
              <Switch />
            </FormField>
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
            <FormField name={getFieldName('prefix_cache_n')} aria-label="Prefix Cache Count">
              <InputNumber
                min={1}
                max={128}
                placeholder="16"
                className="w-full"
              />
            </FormField>
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
            <FormField name={getFieldName('dtype')} aria-label="Data Type">
              <Select
                placeholder="auto"
                className="w-full"
                options={[
                  { value: 'auto', label: 'Auto' },
                  { value: 'f16', label: 'Float16' },
                  { value: 'f32', label: 'Float32' },
                  { value: 'bf16', label: 'BFloat16' },
                ]}
              />
            </FormField>
          </ResponsiveConfigItem>

          <Separator />

          <ResponsiveConfigItem
            title="In-Situ Quantization"
            description={`In-situ quantization to apply (${selectedDeviceType.toUpperCase()})`}
          >
            <FormField name={getFieldName('in_situ_quant')} aria-label="In-Situ Quantization">
              <Select
                placeholder="None"
                className="w-full"
                options={availableQuantizationOptions}
              />
            </FormField>
          </ResponsiveConfigItem>

          <Separator />

          <ResponsiveConfigItem
            title="Random Seed"
            description="Integer seed to ensure reproducible random number generation"
          >
            <FormField name={getFieldName('seed')} aria-label="Random Seed">
              <InputNumber
                min={0}
                max={4294967295}
                placeholder="Random"
                className="w-full"
              />
            </FormField>
          </ResponsiveConfigItem>
        </Flex>
      </Card>
    </Flex>
  )
}
