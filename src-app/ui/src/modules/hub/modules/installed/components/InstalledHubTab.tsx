import { LayoutGrid, Bot, Plug, RotateCw, Trash2 } from 'lucide-react'
import { useState, useMemo, Fragment } from 'react'
import {
  Button,
  Card,
  Separator,
  Empty,
  Flex,
  Confirm,
  Spin,
  Tag,
  Tooltip,
  Text,
  message,
} from '@/components/ui'
import { Stores } from '@/core/stores'
import { ApiClient } from '@/api-client'
import { emitMcpServerDeleted } from '@/modules/mcp/events/emitters'
import { emitAssistantDeleted } from '@/modules/assistant/events/emitters'
import type { HubInstalledRow } from '@/api-client/types'

// Three section cards, data-driven so the row-render loop stays
// flat. Icons match the per-category icon used elsewhere in the
// app (Models = Appstore, Assistants = Robot, MCP = Api) so the
// visual rhythm carries between pages.
const CATEGORY_CARDS: Array<{
  key: HubInstalledRow['hub_category']
  title: string
  icon: React.ReactNode
  emptyHint: string
}> = [
  {
    key: 'model',
    title: 'Models',
    icon: <LayoutGrid />,
    emptyHint:
      'No models installed from the hub yet. Browse the Models tab to install one.',
  },
  {
    key: 'assistant',
    title: 'Assistants',
    icon: <Bot />,
    emptyHint:
      'No assistants installed from the hub yet. Browse the Assistants tab to install one.',
  },
  {
    key: 'mcp_server',
    title: 'MCP Servers',
    icon: <Plug />,
    emptyHint:
      'No MCP servers installed from the hub yet. Browse the MCP Servers tab to install one.',
  },
]

/**
 * Hub Installed tab. Every tracked install the caller can see
 * (own user installs + system installs when admin), grouped into
 * three cards: Models / Assistants / MCP Servers. Each row shows
 * name, scope, install date, installed-vs-current version, and
 * Re-install / Remove actions.
 *
 * Visual structure mirrors the rest of the settings pages
 * (SandboxRootfsVersionsSection, McpServerCard,
 * AuthProvidersListSection) — Card with a simple string title +
 * `extra` for the count, body is a flex column with `<Separator/>`
 * between rows, action buttons right-aligned in the row.
 */
export function InstalledHubTab() {
  const items = Stores.HubInstalled.items
  const loading = Stores.HubInstalled.loading
  const error = Stores.HubInstalled.error
  const catalogVersion = Stores.HubInstalled.catalogVersion
  const [busyId, setBusyId] = useState<string | null>(null)

  // Group rows by hub_category. Done in useMemo so the three cards
  // don't re-filter on every render.
  const grouped = useMemo(() => {
    const map: Record<string, HubInstalledRow[]> = {
      model: [],
      assistant: [],
      mcp_server: [],
    }
    for (const row of items) {
      if (map[row.hub_category]) {
        map[row.hub_category].push(row)
      }
    }
    return map
  }, [items])

  // --- Re-install ---------------------------------------------------------
  //
  // For assistants: `is_template_install` routes through the template
  // endpoint. For MCP servers: `is_system_mcp_install` routes through
  // the system endpoint. User-scoped rows go through `createFromHub`
  // with `replace_existing: true` so the prior user install (and any
  // legacy duplicates accumulated before the replace path existed)
  // get wiped. Models can't be re-installed inline — they need a
  // provider + quantization choice, so the action is disabled with
  // a tooltip pointing at the Models tab.
  const reinstall = async (row: HubInstalledRow) => {
    setBusyId(row.entity_id)
    try {
      if (row.hub_category === 'assistant') {
        if (row.is_template_install) {
          await Stores.HubAssistants.createTemplateFromHub({
            hub_id: row.hub_id,
            replace_existing: true,
          })
        } else {
          await Stores.HubAssistants.createFromHub({
            hub_id: row.hub_id,
            replace_existing: true,
          })
        }
      } else if (row.hub_category === 'mcp_server') {
        if (row.is_system_mcp_install) {
          await Stores.HubMcpServers.createSystemFromHub({
            hub_id: row.hub_id,
            replace_existing: true,
          })
        } else {
          await Stores.HubMcpServers.createFromHub({
            hub_id: row.hub_id,
            replace_existing: true,
          })
        }
      } else if (row.hub_category === 'skill') {
        // Only USER-scope skills reach here — system skills disable the
        // Re-install button (they need group choices; see the button gate).
        // The backend re-install path replaces the prior install for this hub_id.
        await Stores.Skill.installFromHub(row.hub_id)
      } else if (row.hub_category === 'workflow') {
        await Stores.Workflow.installFromHub(row.hub_id)
      } else {
        // Unhandled category — surface an error instead of a false success.
        throw new Error(`Re-install not supported for ${row.hub_category}`)
      }
      message.success(
        `Re-installed ${row.name || row.hub_id} from v${catalogVersion ?? '?'}`,
      )
      await Stores.HubInstalled.loadInstalled()
    } catch (e) {
      message.error(
        `Failed to re-install ${row.hub_id}: ${(e as Error)?.message ?? e}`,
      )
    } finally {
      setBusyId(null)
    }
  }

  // --- Remove -------------------------------------------------------------
  //
  // Deletes the underlying entity (mcp_servers / assistants / llm_models row).
  // The DELETE handlers emit the entity-deleted backend event, which the
  // hub module's `CleanupHubEntitiesHandler` listens to and removes the
  // hub_entities tracking row — so we don't need a separate untrack call.
  // Models: pass `delete_file=true` so the on-disk weights are wiped too,
  // matching the symmetric "Remove = gone" semantic across all categories.
  const remove = async (row: HubInstalledRow) => {
    setBusyId(row.entity_id)
    try {
      if (row.hub_category === 'mcp_server') {
        await ApiClient.McpServer.delete({ id: row.entity_id })
        await emitMcpServerDeleted(row.entity_id)
      } else if (row.hub_category === 'assistant') {
        await ApiClient.Assistant.delete({ id: row.entity_id })
        await emitAssistantDeleted(row.entity_id)
      } else if (row.hub_category === 'model') {
        // delete_file=true wipes on-disk weights too (the handler's
        // default, but explicit here so the intent is obvious).
        // Skipping the llm_model.deleted emit — it needs providerId,
        // which the hub row doesn't carry; downstream stores will
        // pick up the change on next navigation, and the
        // `loadInstalled` reload below refreshes this tab.
        await ApiClient.LlmModel.delete({
          model_id: row.entity_id,
          delete_file: true,
        })
      } else if (row.hub_category === 'skill') {
        if (row.is_system) {
          await ApiClient.SkillSystem.delete({ id: row.entity_id })
        } else {
          await ApiClient.Skill.delete({ id: row.entity_id })
        }
      } else if (row.hub_category === 'workflow') {
        if (row.is_system) {
          await ApiClient.Workflow.deleteSystem({ id: row.entity_id })
        } else {
          await ApiClient.Workflow.delete({ id: row.entity_id })
        }
      } else {
        // Unhandled category — surface an error instead of a false success.
        throw new Error(`Remove not supported for ${row.hub_category}`)
      }
      message.success(`Removed ${row.name || row.hub_id}`)
      await Stores.HubInstalled.loadInstalled()
    } catch (e) {
      message.error(
        `Failed to remove ${row.hub_id}: ${(e as Error)?.message ?? e}`,
      )
    } finally {
      setBusyId(null)
    }
  }

  if (loading && items.length === 0) {
    return (
      <div className="flex justify-center items-center py-12">
        <Spin label="Loading" />
      </div>
    )
  }

  if (error && items.length === 0) {
    return (
      <div className="px-3 pt-3">
        <Empty
          data-testid="hub-installed-error-empty"
          description={
            <Text type="secondary">Couldn't load installed list: {error}</Text>
          }
        />
      </div>
    )
  }

  return (
    <Flex vertical className="gap-3 px-3 pb-6">
      {CATEGORY_CARDS.map(card => {
        const rows = grouped[card.key] ?? []
        return (
          <Card
            key={card.key}
            data-testid={`hub-installed-card-${card.key}`}
            title={
              <Flex align="center" gap="small">
                {card.icon}
                <span>{card.title}</span>
              </Flex>
            }
            extra={
              <Tag data-testid={`hub-installed-count-tag-${card.key}`}>
                {rows.length} {rows.length === 1 ? 'install' : 'installs'}
              </Tag>
            }
          >
            {rows.length === 0 ? (
              <Empty
                data-testid={`hub-installed-empty-${card.key}`}
                description={
                  <Text type="secondary">{card.emptyHint}</Text>
                }
              />
            ) : (
              <Flex vertical className="gap-1">
                {rows.map((row, i) => {
                  const installed = row.installed_version
                  const current = row.current_version
                  const isOutdated = installed !== current
                  const installedAtIso = row.installed_at
                  const installedAtShort = (() => {
                    try {
                      return new Date(installedAtIso).toLocaleDateString()
                    } catch {
                      return installedAtIso
                    }
                  })()
                  const installedAtFull = (() => {
                    try {
                      return new Date(installedAtIso).toLocaleString()
                    } catch {
                      return installedAtIso
                    }
                  })()
                  return (
                    <Fragment key={`${row.entity_type}:${row.entity_id}`}>
                      {i > 0 && <Separator className="!my-3" />}
                      <div className="flex items-start gap-3 flex-wrap" data-testid={`hub-installed-row-${row.entity_id}`}>
                        <div className="flex-1 min-w-48">
                          {/* Title row — name + scope tag + version tag. */}
                          <Flex align="center" gap="small" wrap>
                            <Text className="font-semibold">
                              {row.name || row.hub_id}
                            </Text>
                            {row.is_system && (
                              <Tag tone="info" data-testid={`hub-installed-system-tag-${row.entity_id}`}>System</Tag>
                            )}
                            <Tooltip
                              content={
                                isOutdated
                                  ? `Installed v${installed ?? 'pre-tracking'}; catalog is at v${current}`
                                  : `On catalog v${current}`
                              }
                            >
                              <Tag tone={isOutdated ? 'warning' : 'success'} data-testid={`hub-installed-version-tag-${row.entity_id}`}>
                                {isOutdated
                                  ? `v${installed ?? 'pre-tracking'} → v${current}`
                                  : `v${current}`}
                              </Tag>
                            </Tooltip>
                          </Flex>
                          {/* Subtitle row — hub_id (when different from name)
                              and install date. */}
                          <Flex
                            align="center"
                            gap="small"
                            wrap
                            className="mt-1"
                          >
                            {row.name && row.name !== row.hub_id && (
                              <Text type="secondary" className="text-xs">
                                {row.hub_id}
                              </Text>
                            )}
                            <Tooltip content={`Installed ${installedAtFull}`}>
                              <Text type="secondary" className="text-xs">
                                installed {installedAtShort}
                              </Text>
                            </Tooltip>
                          </Flex>
                        </div>
                        <div className="flex gap-2 items-center justify-end">
                          {row.hub_category === 'model' ||
                          ((row.hub_category === 'skill' ||
                            row.hub_category === 'workflow') &&
                            row.is_system) ? (
                            <Tooltip
                              content={
                                row.hub_category === 'model'
                                  ? 'Models re-install via the Models tab (pick a provider + quantization)'
                                  : `System ${row.hub_category}s re-install from the ${row.hub_category === 'skill' ? 'Skills' : 'Workflows'} tab (it sets the group assignments)`
                              }
                            >
                              <Button icon={<RotateCw />} disabled data-testid={`hub-installed-reinstall-disabled-btn-${row.entity_id}`}>
                                Re-install
                              </Button>
                            </Tooltip>
                          ) : (
                            <Confirm
                              data-testid={`hub-installed-reinstall-confirm-${row.entity_id}`}
                              title="Re-install from current catalog"
                              description={`Re-install "${row.name || row.hub_id}" at v${current}? The existing copy will be replaced.`}
                              okText="Re-install"
                              cancelText="Cancel"
                              onConfirm={() => reinstall(row)}
                            >
                              <Button
                                icon={<RotateCw />}
                                loading={busyId === row.entity_id}
                                data-testid={`hub-installed-reinstall-btn-${row.entity_id}`}
                              >
                                Re-install
                              </Button>
                            </Confirm>
                          )}
                          <Confirm
                            data-testid={`hub-installed-remove-confirm-${row.entity_id}`}
                            title="Remove this install?"
                            description={
                              row.hub_category === 'model'
                                ? `Delete "${row.name || row.hub_id}" and remove the on-disk model files. This can't be undone.`
                                : `Delete "${row.name || row.hub_id}". This can't be undone.`
                            }
                            okText="Remove"
                            okButtonProps={{ danger: true }}
                            cancelText="Cancel"
                            onConfirm={() => remove(row)}
                          >
                            <Button
                              variant="destructive"
                              icon={<Trash2 />}
                              loading={busyId === row.entity_id}
                              data-testid={`hub-installed-remove-btn-${row.entity_id}`}
                            >
                              Remove
                            </Button>
                          </Confirm>
                        </div>
                      </div>
                    </Fragment>
                  )
                })}
              </Flex>
            )}
          </Card>
        )
      })}
    </Flex>
  )
}
