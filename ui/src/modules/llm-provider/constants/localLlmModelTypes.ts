export interface ModelFileType {
  key: string
  label: string
  description: string
  extensions: string[]
  mimeTypes?: string[]
}

// Supported file types for Local models
export const LOCAL_FILE_TYPES: ModelFileType[] = [
  {
    key: 'safetensors',
    label: 'SafeTensors (.safetensors)',
    description:
      'Safe tensor format with metadata validation and memory mapping support',
    extensions: ['.safetensors'],
    mimeTypes: ['application/octet-stream'],
  },
  {
    key: 'pytorch',
    label: 'PyTorch Binary (.bin)',
    description: 'Traditional PyTorch binary format',
    extensions: ['.bin', '.pt', '.pth'],
    mimeTypes: ['application/octet-stream'],
  },
  {
    key: 'gguf',
    label: 'GGUF (.gguf)',
    description: 'GGML Universal Format for quantized models',
    extensions: ['.gguf'],
    mimeTypes: ['application/octet-stream'],
  },
]

// Convert to options format for Select component
export const LOCAL_FILE_TYPE_OPTIONS = LOCAL_FILE_TYPES.map(type => ({
  value: type.key,
  label: type.label,
  description: type.description,
  extensions: type.extensions,
}))
