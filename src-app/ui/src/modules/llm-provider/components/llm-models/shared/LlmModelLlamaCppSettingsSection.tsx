import * as React from 'react'
import {
  Card,
  Separator,
  Flex,
  FormField,
  Input,
  InputNumber,
  Select,
  Switch,
  Text,
} from '@/components/ui'

export function LlmModelLlamaCppSettingsSection() {
  const getFieldName = (field: string) => `engine_settings.llamacpp.${field}`

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
            description="Hardware backend to run the model on. Leave empty to auto-select; CPU forces all layers off the GPU."
          >
            <FormField
              name={getFieldName('device_type')}
              className="m-0 w-[120px]"
            >
              <Select
                placeholder="Auto"
                className="w-full"
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

      {/* Context & Memory Management */}
      <Card title="Context & Memory Management">
        <Flex vertical className="gap-2 w-full">
          <ResponsiveConfigItem
            title="Context Size"
            description="Size of the prompt context (--ctx-size, default: 8192)"
          >
            <FormField
              name={getFieldName('ctx_size')}
              className="m-0 w-[120px]"
            >
              <InputNumber
                min={512}
                max={131072}
                placeholder="8192"
                className="w-full"
              />
            </FormField>
          </ResponsiveConfigItem>

          <Separator />

          <ResponsiveConfigItem
            title="Batch Size"
            description="Logical batch size for prompt processing (--batch-size, default: 2048)"
          >
            <FormField
              name={getFieldName('batch_size')}
              className="m-0 w-[120px]"
            >
              <InputNumber
                min={1}
                max={8192}
                placeholder="2048"
                className="w-full"
              />
            </FormField>
          </ResponsiveConfigItem>

          <Separator />

          <ResponsiveConfigItem
            title="Physical Batch Size"
            description="Physical maximum batch size (--ubatch-size, default: 512)"
          >
            <FormField
              name={getFieldName('ubatch_size')}
              className="m-0 w-[120px]"
            >
              <InputNumber
                min={1}
                max={2048}
                placeholder="512"
                className="w-full"
              />
            </FormField>
          </ResponsiveConfigItem>

          <Separator />

          <ResponsiveConfigItem
            title="Parallel Sequences"
            description="Number of parallel sequences to process (--parallel, default: 1)"
          >
            <FormField
              name={getFieldName('parallel')}
              className="m-0 w-[120px]"
            >
              <InputNumber
                min={1}
                max={64}
                placeholder="1"
                className="w-full"
              />
            </FormField>
          </ResponsiveConfigItem>

          <Separator />

          <ResponsiveConfigItem
            title="Keep Tokens"
            description="Tokens to keep from initial prompt (--keep, default: 0)"
          >
            <FormField
              name={getFieldName('keep')}
              className="m-0 w-[120px]"
            >
              <InputNumber
                min={0}
                max={4096}
                placeholder="0"
                className="w-full"
              />
            </FormField>
          </ResponsiveConfigItem>

          <Separator />

          <ResponsiveConfigItem
            title="Memory Lock"
            description="Lock the model in memory, preventing it from being swapped out (--mlock)"
          >
            <FormField
              name={getFieldName('mlock')}
              valuePropName="checked"
              className="m-0"
            >
              <Switch />
            </FormField>
          </ResponsiveConfigItem>

          <Separator />

          <ResponsiveConfigItem
            title="Disable Memory Mapping"
            description="Disable memory mapping for model files (--no-mmap)"
          >
            <FormField
              name={getFieldName('no_mmap')}
              valuePropName="checked"
              className="m-0"
            >
              <Switch />
            </FormField>
          </ResponsiveConfigItem>
        </Flex>
      </Card>

      {/* Threading & Performance */}
      <Card title="Threading & Performance">
        <Flex vertical className="gap-2 w-full">
          <ResponsiveConfigItem
            title="Generation Threads"
            description="Number of threads to use for generation (--threads, default: auto)"
          >
            <FormField
              name={getFieldName('threads')}
              className="m-0 w-[120px]"
            >
              <InputNumber
                min={-1}
                max={64}
                placeholder="-1"
                className="w-full"
              />
            </FormField>
          </ResponsiveConfigItem>

          <Separator />

          <ResponsiveConfigItem
            title="Batch Processing Threads"
            description="Number of threads for batch processing (--threads-batch, default: same as threads)"
          >
            <FormField
              name={getFieldName('threads_batch')}
              className="m-0 w-[120px]"
            >
              <InputNumber
                min={1}
                max={64}
                placeholder="Auto"
                className="w-full"
              />
            </FormField>
          </ResponsiveConfigItem>

          <Separator />

          <ResponsiveConfigItem
            title="Continuous Batching"
            description="Enable continuous batching for better throughput (--cont-batching)"
          >
            <FormField
              name={getFieldName('cont_batching')}
              valuePropName="checked"
              className="m-0"
            >
              <Switch />
            </FormField>
          </ResponsiveConfigItem>

          <Separator />

          <ResponsiveConfigItem
            title="Flash Attention"
            description="Enable Flash Attention for faster inference (--flash-attn)"
          >
            <FormField
              name={getFieldName('flash_attn')}
              valuePropName="checked"
              className="m-0"
            >
              <Switch />
            </FormField>
          </ResponsiveConfigItem>

          <Separator />

          <ResponsiveConfigItem
            title="Disable KV Offload"
            description="Disable KV cache offloading to GPU (--no-kv-offload)"
          >
            <FormField
              name={getFieldName('no_kv_offload')}
              valuePropName="checked"
              className="m-0"
            >
              <Switch />
            </FormField>
          </ResponsiveConfigItem>
        </Flex>
      </Card>

      {/* GPU Configuration */}
      <Card title="GPU Configuration">
        <Flex vertical className="gap-2 w-full">
          <ResponsiveConfigItem
            title="GPU Layers"
            description="Number of layers to offload to GPU (--n-gpu-layers, default: 0)"
          >
            <FormField
              name={getFieldName('n_gpu_layers')}
              className="m-0 w-[120px]"
            >
              <InputNumber
                min={0}
                max={128}
                placeholder="0"
                className="w-full"
              />
            </FormField>
          </ResponsiveConfigItem>

          <Separator />

          <ResponsiveConfigItem
            title="Main GPU"
            description="Primary GPU index to use (--main-gpu, default: 0)"
          >
            <FormField
              name={getFieldName('main_gpu')}
              className="m-0 w-[120px]"
            >
              <InputNumber
                min={0}
                max={16}
                placeholder="0"
                className="w-full"
              />
            </FormField>
          </ResponsiveConfigItem>

          <Separator />

          <ResponsiveConfigItem
            title="Split Mode"
            description="How to split the model across multiple GPUs (--split-mode)"
          >
            <FormField
              name={getFieldName('split_mode')}
              className="m-0 w-[120px]"
            >
              <Select
                placeholder="none"
                className="w-full"
                options={[
                  { value: 'none', label: 'None' },
                  { value: 'layer', label: 'Layer' },
                  { value: 'row', label: 'Row' },
                ]}
              />
            </FormField>
          </ResponsiveConfigItem>

          <Separator />

          <ResponsiveConfigItem
            title="Tensor Split"
            description="GPU memory distribution ratios (--tensor-split, e.g., '3,1')"
          >
            <FormField
              name={getFieldName('tensor_split')}
              className="m-0 w-[120px]"
            >
              <Input placeholder="3,1" className="w-full" />
            </FormField>
          </ResponsiveConfigItem>
        </Flex>
      </Card>

      {/* Model Configuration */}
      <Card title="Model Configuration">
        <Flex vertical className="gap-2 w-full">
          <ResponsiveConfigItem
            title="RoPE Base Frequency"
            description="RoPE base frequency (--rope-freq-base, default: auto)"
          >
            <FormField
              name={getFieldName('rope_freq_base')}
              className="m-0 w-[120px]"
            >
              <InputNumber
                min={1000}
                max={1000000}
                placeholder="Auto"
                className="w-full"
              />
            </FormField>
          </ResponsiveConfigItem>

          <Separator />

          <ResponsiveConfigItem
            title="RoPE Frequency Scale"
            description="RoPE frequency scaling factor (--rope-freq-scale, default: auto)"
          >
            <FormField
              name={getFieldName('rope_freq_scale')}
              className="m-0 w-[120px]"
            >
              <InputNumber
                min={0.1}
                max={10.0}
                step={0.1}
                placeholder="Auto"
                className="w-full"
              />
            </FormField>
          </ResponsiveConfigItem>

          <Separator />

          <ResponsiveConfigItem
            title="RoPE Scaling"
            description="RoPE scaling method (--rope-scaling)"
          >
            <FormField
              name={getFieldName('rope_scaling')}
              className="m-0 w-[120px]"
            >
              <Select
                placeholder="none"
                className="w-full"
                options={[
                  { value: 'none', label: 'None' },
                  { value: 'linear', label: 'Linear' },
                  { value: 'yarn', label: 'YaRN' },
                ]}
              />
            </FormField>
          </ResponsiveConfigItem>

          <Separator />

          <ResponsiveConfigItem
            title="KV Cache Type (K)"
            description="KV cache data type for K (--cache-type-k, e.g., f16, f32, q8_0)"
          >
            <FormField
              name={getFieldName('cache_type_k')}
              className="m-0 w-[120px]"
            >
              <Select
                placeholder="f16"
                className="w-full"
                options={[
                  { value: 'f16', label: 'f16' },
                  { value: 'f32', label: 'f32' },
                  { value: 'q8_0', label: 'q8_0' },
                  { value: 'q4_0', label: 'q4_0' },
                ]}
              />
            </FormField>
          </ResponsiveConfigItem>

          <Separator />

          <ResponsiveConfigItem
            title="KV Cache Type (V)"
            description="KV cache data type for V (--cache-type-v, e.g., f16, f32, q8_0)"
          >
            <FormField
              name={getFieldName('cache_type_v')}
              className="m-0 w-[120px]"
            >
              <Select
                placeholder="f16"
                className="w-full"
                options={[
                  { value: 'f16', label: 'f16' },
                  { value: 'f32', label: 'f32' },
                  { value: 'q8_0', label: 'q8_0' },
                  { value: 'q4_0', label: 'q4_0' },
                ]}
              />
            </FormField>
          </ResponsiveConfigItem>
        </Flex>
      </Card>

      {/* Advanced Options */}
      <Card title="Advanced Options">
        <Flex vertical className="gap-2 w-full">
          <ResponsiveConfigItem
            title="Random Seed"
            description="Seed for random number generation (--seed, -1 for random)"
          >
            <FormField
              name={getFieldName('seed')}
              className="m-0 w-[120px]"
            >
              <InputNumber
                min={-1}
                max={4294967295}
                placeholder="-1"
                className="w-full"
              />
            </FormField>
          </ResponsiveConfigItem>

          <Separator />

          <ResponsiveConfigItem
            title="NUMA Optimization"
            description="NUMA optimizations (--numa)"
          >
            <FormField
              name={getFieldName('numa')}
              className="m-0 w-[120px]"
            >
              <Select
                placeholder="None"
                className="w-full"
                options={[
                  { value: 'distribute', label: 'Distribute' },
                  { value: 'isolate', label: 'Isolate' },
                  { value: 'numactl', label: 'Numactl' },
                ]}
              />
            </FormField>
          </ResponsiveConfigItem>
        </Flex>
      </Card>
    </Flex>
  )
}
