import { McpServerFormData } from './form-helpers'

/**
 * Pre-built McpServerFormData variants for tests that need a stock
 * "sampling-enabled HTTP server", "built-in server", etc. The unique-name
 * suffix prevents collisions when multiple specs run in parallel against
 * the same per-worker DB.
 */

function uniqueName(base: string): string {
  const suffix = `${Date.now()}-${Math.floor(Math.random() * 1e6)}`
  return `${base}_${suffix}`
}

/** HTTP server, sampling on, usage_mode='auto', max_concurrent_sessions=3. */
export function samplingHttpServer(
  overrides: Partial<McpServerFormData> = {},
): McpServerFormData {
  const name = uniqueName('test_sampling_http')
  return {
    name,
    displayName: 'Test Sampling HTTP',
    description: 'HTTP server with sampling enabled (e2e fixture)',
    transportType: 'http',
    url: 'https://example.invalid/mcp',
    enabled: true,
    supportsSampling: true,
    usageMode: 'auto',
    maxConcurrentSessions: 3,
    ...overrides,
  }
}

/** HTTP server, sampling OFF — baseline for "no badge" assertions. */
export function nonSamplingHttpServer(
  overrides: Partial<McpServerFormData> = {},
): McpServerFormData {
  const name = uniqueName('test_basic_http')
  return {
    name,
    displayName: 'Test Basic HTTP',
    description: 'HTTP server without sampling (e2e fixture)',
    transportType: 'http',
    url: 'https://example.invalid/mcp',
    enabled: true,
    supportsSampling: false,
    ...overrides,
  }
}

/** Sampling-on server with usage_mode='always' — triggers BOTH the
 *  "Sampling" and "Always" badges on the card. */
export function alwaysSamplingHttpServer(
  overrides: Partial<McpServerFormData> = {},
): McpServerFormData {
  return samplingHttpServer({
    name: uniqueName('test_always_sampling_http'),
    displayName: 'Test Always-Sampling HTTP',
    usageMode: 'always',
    ...overrides,
  })
}

/** Stdio server, no sampling — baseline for stdio CRUD specs. */
export function basicStdioServer(
  overrides: Partial<McpServerFormData> = {},
): McpServerFormData {
  const name = uniqueName('test_basic_stdio')
  return {
    name,
    displayName: 'Test Basic Stdio',
    description: 'Stdio server (e2e fixture)',
    transportType: 'stdio',
    command: 'node',
    args: ['server.js'],
    env: { NODE_ENV: 'test' },
    enabled: true,
    ...overrides,
  }
}

/**
 * Raw API payload (snake_case, server-side shape) for the system server
 * POST endpoint. Used by tests that bypass the UI to seed a server via API.
 * Mirrors the form payload constructed by McpServerDrawer.
 */
export function systemServerApiPayload(
  data: McpServerFormData,
  extras: { is_built_in?: boolean } = {},
): Record<string, unknown> {
  const base: Record<string, unknown> = {
    name: data.name,
    display_name: data.displayName,
    description: data.description ?? null,
    enabled: data.enabled ?? true,
    transport_type: data.transportType,
    supports_sampling: data.supportsSampling ?? false,
    usage_mode: data.usageMode ?? 'auto',
    max_concurrent_sessions: data.maxConcurrentSessions ?? null,
    timeout_seconds: 30,
  }
  if (data.transportType === 'stdio') {
    base.command = data.command
    base.args = data.args ?? []
    base.environment_variables = data.env ?? {}
  } else {
    base.url = data.url
  }
  if (extras.is_built_in !== undefined) {
    base.is_built_in = extras.is_built_in
  }
  return base
}
