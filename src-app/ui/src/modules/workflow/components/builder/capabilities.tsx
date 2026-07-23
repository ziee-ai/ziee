import { useMemo } from 'react'
import { MultiSelect, Select } from '@ziee/kit'
import { McpServer } from '@/modules/mcp/stores/mcpServer'

// ---------------------------------------------------------------------------
// Capability pickers over the user's accessible MCP servers. `McpServer`
// is the sanctioned cross-module accessor. A workflow step's `servers` / `tool`
// step `server` resolves by NAME at run time (see `resolve_tool_server`), so the
// option VALUE is the server name while the LABEL is its friendly display name.
// ---------------------------------------------------------------------------

function useCapabilityOptions(): { value: string; label: string }[] {
  const servers = McpServer.servers
  return useMemo(
    () =>
      (servers ?? [])
        .filter(s => s.enabled)
        .map(s => ({ value: s.name, label: s.display_name || s.name })),
    [servers],
  )
}

interface CapabilityMultiSelectProps {
  value: string[]
  onChange: (value: string[]) => void
  testid: string
  placeholder?: string
}

/** Multi-pick of capabilities (server names). Empty catalog → a clear note. */
export function CapabilityMultiSelect({
  value,
  onChange,
  testid,
  placeholder = 'Add a capability',
}: CapabilityMultiSelectProps) {
  const options = useCapabilityOptions()
  return (
    <MultiSelect
      data-testid={testid}
      aria-label="Capabilities"
      options={options}
      value={value}
      onChange={onChange}
      placeholder={placeholder}
      searchPlaceholder="Search capabilities…"
      emptyText="No tools available"
      removeLabel={v => `Remove ${v}`}
    />
  )
}

interface CapabilitySelectProps {
  value: string
  onChange: (value: string) => void
  testid: string
  placeholder?: string
}

/** Single-pick of a capability (server name) — used by the `tool` step. */
export function CapabilitySelect({
  value,
  onChange,
  testid,
  placeholder = 'Select a server',
}: CapabilitySelectProps) {
  const options = useCapabilityOptions()
  return (
    <Select
      data-testid={testid}
      aria-label="Server"
      options={options}
      value={value || undefined}
      onChange={onChange}
      placeholder={options.length === 0 ? 'No servers available' : placeholder}
      popupMatchSelectWidth={false}
    />
  )
}
