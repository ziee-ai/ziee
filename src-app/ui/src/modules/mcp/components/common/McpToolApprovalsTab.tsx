import { useEffect, useState } from 'react'
import { Alert, Card, Empty, Select, Text, message } from '@ziee/kit'
import {
  Field,
  FieldContent,
  FieldDescription,
  FieldGroup,
  FieldTitle,
} from '@ziee/kit/shadcn/field'
import { ApiClient } from '@/api-client'
import type {
  ApprovalMode,
  ServerToolApprovalsResponse,
  ToolApprovalEntry,
} from '@/api-client/types'

/** Human labels for the three admin approval modes (the `ApprovalMode` vocab). */
const MODE_LABEL: Record<ApprovalMode, string> = {
  auto_approve: 'Auto-approve',
  manual_approve: 'Manual approve',
  disabled: 'Disabled',
}

/** Sentinel Select value for "no override → fall back to the server default".
 *  Selecting it clears the per-tool override (PUT with no `mode`). */
const USE_DEFAULT = '__default__'

/**
 * Per-tool approval overrides for a SYSTEM MCP server. Rendered as a tab inside
 * McpServerDrawer (edit-system mode only — the backend PUT is system-only).
 *
 * Lists every advertised tool with a Select choosing its admin approval mode
 * (Auto-approve / Manual approve / Disabled, or "Use server default" to clear an
 * override). When the live `tools/list` probe fails the response carries
 * `tools_unreachable` — a clear warning is shown (with `unreachable_reason`)
 * instead of a silent empty list; any tools that still carry an override are
 * listed below it.
 *
 * Self-contained fetch (no store / no sync entity): admin config edited in place,
 * with the returned `effective_mode`/`has_override` folded back into local state.
 */
export function McpToolApprovalsTab({
  serverId,
  canManage,
}: {
  serverId: string
  canManage: boolean
}) {
  const [data, setData] = useState<ServerToolApprovalsResponse | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [savingTool, setSavingTool] = useState<string | null>(null)

  // (Re)load this server's tool approvals on mount / when the drawer swaps servers.
  useEffect(() => {
    let cancelled = false
    setLoading(true)
    setError(null)
    ApiClient.McpServerToolApprovals.get({ id: serverId })
      .then(resp => {
        if (!cancelled) setData(resp)
      })
      .catch(e => {
        if (!cancelled) {
          setError(e instanceof Error ? e.message : 'Failed to load tool approvals')
        }
      })
      .finally(() => {
        if (!cancelled) setLoading(false)
      })
    return () => {
      cancelled = true
    }
  }, [serverId])

  const handleChange = async (entry: ToolApprovalEntry, value: string) => {
    // `USE_DEFAULT` clears the override (PUT with no `mode`); otherwise set the mode.
    const mode = value === USE_DEFAULT ? undefined : (value as ApprovalMode)
    setSavingTool(entry.tool_name)
    try {
      const resp = await ApiClient.McpServerToolApprovals.set({
        id: serverId,
        tool: entry.tool_name,
        mode,
      })
      setData(prev =>
        prev
          ? {
              ...prev,
              tools: prev.tools.map(t =>
                t.tool_name === entry.tool_name
                  ? {
                      ...t,
                      effective_mode: resp.effective_mode,
                      has_override: resp.has_override,
                    }
                  : t,
              ),
            }
          : prev,
      )
      message.success(
        mode === undefined
          ? `"${entry.tool_name}" now uses the server default`
          : `"${entry.tool_name}" set to ${MODE_LABEL[mode]}`,
      )
    } catch (e) {
      message.error(e instanceof Error ? e.message : 'Failed to update tool approval')
    } finally {
      setSavingTool(null)
    }
  }

  return (
    <Card
      title="Tool approvals"
      size="sm"
      loading={loading}
      data-testid="mcp-tool-approvals-card"
    >
      {error ? (
        <Alert
          tone="error"
          title="Couldn't load tool approvals"
          description={error}
          data-testid="mcp-tool-approvals-error"
        />
      ) : data ? (
        <div className="flex flex-col gap-4">
          <Text type="secondary" className="text-sm">
            Control how each tool this server advertises is approved before the
            agent may call it. Tools without an explicit override fall back to the
            server default ({MODE_LABEL[data.server_default_mode]}).
          </Text>

          {data.tools_unreachable ? (
            <Alert
              tone="warning"
              title="Server unreachable — can't list tools"
              description={
                data.unreachable_reason ??
                'The server did not respond to a tools/list probe. Only tools that already have an override are shown.'
              }
              data-testid="mcp-tool-approvals-unreachable"
            />
          ) : null}

          {data.tools.length > 0 ? (
            <FieldGroup data-testid="mcp-tool-approvals-list">
              {data.tools.map(entry => (
                <Field key={entry.tool_name} orientation="responsive">
                  <FieldContent>
                    <FieldTitle>{entry.tool_name}</FieldTitle>
                    {entry.description ? (
                      <FieldDescription>{entry.description}</FieldDescription>
                    ) : null}
                  </FieldContent>
                  <Select
                    className="min-w-[180px]"
                    aria-label={`Approval mode for ${entry.tool_name}`}
                    data-testid={`mcp-tool-approval-select-${entry.tool_name}`}
                    disabled={!canManage}
                    loading={savingTool === entry.tool_name}
                    value={entry.has_override ? entry.effective_mode : USE_DEFAULT}
                    onChange={value => void handleChange(entry, value)}
                    options={[
                      {
                        value: USE_DEFAULT,
                        label: `Use server default (${MODE_LABEL[data.server_default_mode]})`,
                      },
                      { value: 'auto_approve', label: MODE_LABEL.auto_approve },
                      { value: 'manual_approve', label: MODE_LABEL.manual_approve },
                      { value: 'disabled', label: MODE_LABEL.disabled },
                    ]}
                  />
                </Field>
              ))}
            </FieldGroup>
          ) : !data.tools_unreachable ? (
            <Empty
              description="This server advertises no tools."
              data-testid="mcp-tool-approvals-empty"
            />
          ) : null}
        </div>
      ) : null}
    </Card>
  )
}
