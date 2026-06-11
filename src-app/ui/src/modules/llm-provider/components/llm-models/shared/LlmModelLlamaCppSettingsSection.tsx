import {
  Card,
  Divider,
  Flex,
  Form,
  Input,
  InputNumber,
  Select,
  Switch,
  Typography,
} from 'antd'

const { Text } = Typography

export function LlmModelLlamaCppSettingsSection() {
  const getFieldName = (field: string) => ['engine_settings', 'llamacpp', field]

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
      {/* Device */}
      <Card title="Device">
        <Flex vertical className="gap-2 w-full">
          <ResponsiveConfigItem
            title="Device Type"
            description="Hardware backend to run the model on. Leave empty to auto-select; CPU forces all layers off the GPU."
          >
            <Form.Item
              name={getFieldName('device_type')}
              style={{ margin: 0, width: 120 }}
            >
              <Select
                placeholder="Auto"
                style={{ width: '100%' }}
                allowClear
                options={[
                  { value: 'cpu', label: 'CPU' },
                  { value: 'cuda', label: 'CUDA' },
                  { value: 'metal', label: 'Metal' },
                  { value: 'rocm', label: 'ROCm' },
                  { value: 'vulkan', label: 'Vulkan' },
                ]}
              />
            </Form.Item>
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
            <Form.Item
              name={getFieldName('ctx_size')}
              style={{ margin: 0, width: 120 }}
            >
              <InputNumber
                min={512}
                max={131072}
                placeholder="8192"
                style={{ width: '100%' }}
              />
            </Form.Item>
          </ResponsiveConfigItem>

          <Divider style={{ margin: 0 }} />

          <ResponsiveConfigItem
            title="Batch Size"
            description="Logical batch size for prompt processing (--batch-size, default: 2048)"
          >
            <Form.Item
              name={getFieldName('batch_size')}
              style={{ margin: 0, width: 120 }}
            >
              <InputNumber
                min={1}
                max={8192}
                placeholder="2048"
                style={{ width: '100%' }}
              />
            </Form.Item>
          </ResponsiveConfigItem>

          <Divider style={{ margin: 0 }} />

          <ResponsiveConfigItem
            title="Physical Batch Size"
            description="Physical maximum batch size (--ubatch-size, default: 512)"
          >
            <Form.Item
              name={getFieldName('ubatch_size')}
              style={{ margin: 0, width: 120 }}
            >
              <InputNumber
                min={1}
                max={2048}
                placeholder="512"
                style={{ width: '100%' }}
              />
            </Form.Item>
          </ResponsiveConfigItem>

          <Divider style={{ margin: 0 }} />

          <ResponsiveConfigItem
            title="Parallel Sequences"
            description="Number of parallel sequences to process (--parallel, default: 1)"
          >
            <Form.Item
              name={getFieldName('parallel')}
              style={{ margin: 0, width: 120 }}
            >
              <InputNumber
                min={1}
                max={64}
                placeholder="1"
                style={{ width: '100%' }}
              />
            </Form.Item>
          </ResponsiveConfigItem>

          <Divider style={{ margin: 0 }} />

          <ResponsiveConfigItem
            title="Keep Tokens"
            description="Tokens to keep from initial prompt (--keep, default: 0)"
          >
            <Form.Item
              name={getFieldName('keep')}
              style={{ margin: 0, width: 120 }}
            >
              <InputNumber
                min={0}
                max={4096}
                placeholder="0"
                style={{ width: '100%' }}
              />
            </Form.Item>
          </ResponsiveConfigItem>

          <Divider style={{ margin: 0 }} />

          <ResponsiveConfigItem
            title="Memory Lock"
            description="Lock the model in memory, preventing it from being swapped out (--mlock)"
          >
            <Form.Item
              name={getFieldName('mlock')}
              valuePropName="checked"
              style={{ margin: 0 }}
            >
              <Switch />
            </Form.Item>
          </ResponsiveConfigItem>

          <Divider style={{ margin: 0 }} />

          <ResponsiveConfigItem
            title="Disable Memory Mapping"
            description="Disable memory mapping for model files (--no-mmap)"
          >
            <Form.Item
              name={getFieldName('no_mmap')}
              valuePropName="checked"
              style={{ margin: 0 }}
            >
              <Switch />
            </Form.Item>
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
            <Form.Item
              name={getFieldName('threads')}
              style={{ margin: 0, width: 120 }}
            >
              <InputNumber
                min={-1}
                max={64}
                placeholder="-1"
                style={{ width: '100%' }}
              />
            </Form.Item>
          </ResponsiveConfigItem>

          <Divider style={{ margin: 0 }} />

          <ResponsiveConfigItem
            title="Batch Processing Threads"
            description="Number of threads for batch processing (--threads-batch, default: same as threads)"
          >
            <Form.Item
              name={getFieldName('threads_batch')}
              style={{ margin: 0, width: 120 }}
            >
              <InputNumber
                min={1}
                max={64}
                placeholder="Auto"
                style={{ width: '100%' }}
              />
            </Form.Item>
          </ResponsiveConfigItem>

          <Divider style={{ margin: 0 }} />

          <ResponsiveConfigItem
            title="Continuous Batching"
            description="Enable continuous batching for better throughput (--cont-batching)"
          >
            <Form.Item
              name={getFieldName('cont_batching')}
              valuePropName="checked"
              style={{ margin: 0 }}
            >
              <Switch />
            </Form.Item>
          </ResponsiveConfigItem>

          <Divider style={{ margin: 0 }} />

          <ResponsiveConfigItem
            title="Flash Attention"
            description="Enable Flash Attention for faster inference (--flash-attn)"
          >
            <Form.Item
              name={getFieldName('flash_attn')}
              valuePropName="checked"
              style={{ margin: 0 }}
            >
              <Switch />
            </Form.Item>
          </ResponsiveConfigItem>

          <Divider style={{ margin: 0 }} />

          <ResponsiveConfigItem
            title="Disable KV Offload"
            description="Disable KV cache offloading to GPU (--no-kv-offload)"
          >
            <Form.Item
              name={getFieldName('no_kv_offload')}
              valuePropName="checked"
              style={{ margin: 0 }}
            >
              <Switch />
            </Form.Item>
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
            <Form.Item
              name={getFieldName('n_gpu_layers')}
              style={{ margin: 0, width: 120 }}
            >
              <InputNumber
                min={0}
                max={128}
                placeholder="0"
                style={{ width: '100%' }}
              />
            </Form.Item>
          </ResponsiveConfigItem>

          <Divider style={{ margin: 0 }} />

          <ResponsiveConfigItem
            title="Main GPU"
            description="Primary GPU index to use (--main-gpu, default: 0)"
          >
            <Form.Item
              name={getFieldName('main_gpu')}
              style={{ margin: 0, width: 120 }}
            >
              <InputNumber
                min={0}
                max={16}
                placeholder="0"
                style={{ width: '100%' }}
              />
            </Form.Item>
          </ResponsiveConfigItem>

          <Divider style={{ margin: 0 }} />

          <ResponsiveConfigItem
            title="Split Mode"
            description="How to split the model across multiple GPUs (--split-mode)"
          >
            <Form.Item
              name={getFieldName('split_mode')}
              style={{ margin: 0, width: 120 }}
            >
              <Select
                placeholder="none"
                style={{ width: '100%' }}
                allowClear
                options={[
                  { value: 'none', label: 'None' },
                  { value: 'layer', label: 'Layer' },
                  { value: 'row', label: 'Row' },
                ]}
              />
            </Form.Item>
          </ResponsiveConfigItem>

          <Divider style={{ margin: 0 }} />

          <ResponsiveConfigItem
            title="Tensor Split"
            description="GPU memory distribution ratios (--tensor-split, e.g., '3,1')"
          >
            <Form.Item
              name={getFieldName('tensor_split')}
              style={{ margin: 0, width: 120 }}
            >
              <Input placeholder="3,1" style={{ width: '100%' }} />
            </Form.Item>
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
            <Form.Item
              name={getFieldName('rope_freq_base')}
              style={{ margin: 0, width: 120 }}
            >
              <InputNumber
                min={1000}
                max={1000000}
                placeholder="Auto"
                style={{ width: '100%' }}
              />
            </Form.Item>
          </ResponsiveConfigItem>

          <Divider style={{ margin: 0 }} />

          <ResponsiveConfigItem
            title="RoPE Frequency Scale"
            description="RoPE frequency scaling factor (--rope-freq-scale, default: auto)"
          >
            <Form.Item
              name={getFieldName('rope_freq_scale')}
              style={{ margin: 0, width: 120 }}
            >
              <InputNumber
                min={0.1}
                max={10.0}
                step={0.1}
                placeholder="Auto"
                style={{ width: '100%' }}
              />
            </Form.Item>
          </ResponsiveConfigItem>

          <Divider style={{ margin: 0 }} />

          <ResponsiveConfigItem
            title="RoPE Scaling"
            description="RoPE scaling method (--rope-scaling)"
          >
            <Form.Item
              name={getFieldName('rope_scaling')}
              style={{ margin: 0, width: 120 }}
            >
              <Select
                placeholder="none"
                style={{ width: '100%' }}
                allowClear
                options={[
                  { value: 'none', label: 'None' },
                  { value: 'linear', label: 'Linear' },
                  { value: 'yarn', label: 'YaRN' },
                ]}
              />
            </Form.Item>
          </ResponsiveConfigItem>

          <Divider style={{ margin: 0 }} />

          <ResponsiveConfigItem
            title="KV Cache Type (K)"
            description="KV cache data type for K (--cache-type-k, e.g., f16, f32, q8_0)"
          >
            <Form.Item
              name={getFieldName('cache_type_k')}
              style={{ margin: 0, width: 120 }}
            >
              <Select
                placeholder="f16"
                style={{ width: '100%' }}
                allowClear
                options={[
                  { value: 'f16', label: 'f16' },
                  { value: 'f32', label: 'f32' },
                  { value: 'q8_0', label: 'q8_0' },
                  { value: 'q4_0', label: 'q4_0' },
                ]}
              />
            </Form.Item>
          </ResponsiveConfigItem>

          <Divider style={{ margin: 0 }} />

          <ResponsiveConfigItem
            title="KV Cache Type (V)"
            description="KV cache data type for V (--cache-type-v, e.g., f16, f32, q8_0)"
          >
            <Form.Item
              name={getFieldName('cache_type_v')}
              style={{ margin: 0, width: 120 }}
            >
              <Select
                placeholder="f16"
                style={{ width: '100%' }}
                allowClear
                options={[
                  { value: 'f16', label: 'f16' },
                  { value: 'f32', label: 'f32' },
                  { value: 'q8_0', label: 'q8_0' },
                  { value: 'q4_0', label: 'q4_0' },
                ]}
              />
            </Form.Item>
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
            <Form.Item
              name={getFieldName('seed')}
              style={{ margin: 0, width: 120 }}
            >
              <InputNumber
                min={-1}
                max={4294967295}
                placeholder="-1"
                style={{ width: '100%' }}
              />
            </Form.Item>
          </ResponsiveConfigItem>

          <Divider style={{ margin: 0 }} />

          <ResponsiveConfigItem
            title="NUMA Optimization"
            description="NUMA optimizations (--numa)"
          >
            <Form.Item
              name={getFieldName('numa')}
              style={{ margin: 0, width: 120 }}
            >
              <Select
                placeholder="None"
                style={{ width: '100%' }}
                allowClear
                options={[
                  { value: 'distribute', label: 'Distribute' },
                  { value: 'isolate', label: 'Isolate' },
                  { value: 'numactl', label: 'Numactl' },
                ]}
              />
            </Form.Item>
          </ResponsiveConfigItem>
        </Flex>
      </Card>
    </Flex>
  )
}
