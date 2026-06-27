import { Card, Tag, Tooltip, Text, Button, Flex, message } from '@/components/ui'
import {
  DownloadOutlined,
  GlobalOutlined,
  GithubOutlined,
  EyeOutlined,
  CopyOutlined,
} from '@ant-design/icons'
import {
  Permissions,
  type HubMCPServer,
  type TransportType,
} from '@/api-client/types'
import { useState } from 'react'
import { McpServerDetailsDrawer } from '@/modules/hub/modules/mcp/components/McpServerDetailsDrawer'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { useNavigate } from 'react-router-dom'
import type { McpServerDrawerPrefill } from '@/modules/mcp/stores/McpServerDrawer.store'

interface McpServerHubCardProps {
  server: HubMCPServer
}

/// Derive the ziee MCP server slug (`^[a-z0-9-]+$`) from the
/// reverse-DNS `name` — take the leaf after the FIRST `/`, lowercase
/// + replace non-`[a-z0-9-]` with `-`. Mirrors the backend's
/// `derive_mcp_slug` so the card's prefill matches what the install
/// handler would compute.
function deriveSlug(name: string): string {
  const slash = name.indexOf('/')
  const leaf = slash >= 0 ? name.slice(slash + 1) : name
  return leaf
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 63)
}

export function McpServerHubCard({ server }: McpServerHubCardProps) {
  const navigate = useNavigate()
  const [showDetails, setShowDetails] = useState(false)
  const [installing, setInstalling] = useState(false)
  const [installingSystem, setInstallingSystem] = useState(false)
  const canInstall = usePermission(Permissions.HubMcpServersCreate)
  const canInstallSystem = usePermission(Permissions.McpServersAdminCreate)
  const { multiUserMode } = Stores.AppMode

  const slug = deriveSlug(server.name)
  // Leaf (after the first `/` in the reverse-DNS name) — used only as
  // the display fallback when the catalog's IndexItem has no curated
  // title. Publishers set the title via `_hub_curation.title` in the
  // source YAML; the build script promotes it to `IndexItem.title`.
  const leaf = (() => {
    const slash = server.name.indexOf('/')
    return slash >= 0 ? server.name.slice(slash + 1) : server.name
  })()
  // Prefer the catalog-curated title over the bare leaf. Subscribe to
  // `Stores.HubCatalog.catalog` so the title updates when the catalog
  // refreshes (e.g. mid-test against the mock Pages server).
  const indexItem = Stores.HubCatalog.catalog?.items.find(
    it => it.category === 'mcp-server' && it.name === server.name,
  )
  const displayTitle = indexItem?.title ?? leaf

  // Pick the first package / remote so the card can show a transport
  // tag without re-deriving in two places.
  const firstRemote = server.remotes && server.remotes[0]
  const firstPackage = server.packages && server.packages[0]
  const transportLabel: string = firstRemote
    ? (firstRemote.type ?? 'remote').toUpperCase()
    : firstPackage
      ? (firstPackage.transport?.type ?? 'stdio').toUpperCase()
      : 'STDIO'

  const isAlreadyInstalled = server.created_ids && server.created_ids.length > 0
  const isAlreadyInstalledAsSystem =
    server.created_system_ids && server.created_system_ids.length > 0

  /**
   * Translate the strict server.json into the McpServerDrawer's
   * prefill. The backend's install path mirrors this derivation in
   * `build_mcp_server_create_from_hub` — keep them in sync.
   */
  const prefillFromHub = (): McpServerDrawerPrefill => {
    let transport: TransportType = 'stdio'
    let command: string | undefined = undefined
    let args: string[] | undefined = undefined
    let url: string | undefined = undefined
    const envEntries: {
      key: string
      value: string
      is_secret: boolean
    }[] = []
    const headerEntries: {
      key: string
      value: string
      is_secret: boolean
    }[] = []

    if (firstRemote) {
      transport = firstRemote.type === 'sse' ? 'sse' : 'http'
      url = firstRemote.url ?? undefined
      for (const h of firstRemote.headers ?? []) {
        headerEntries.push({
          key: h.name,
          value: String(h.value ?? h.default ?? ''),
          is_secret: !!h.isSecret,
        })
      }
    } else if (firstPackage) {
      transport = 'stdio'
      command = firstPackage.runtimeHint ?? undefined
      const argv: string[] = []
      for (const a of firstPackage.runtimeArguments ?? []) {
        if (a.value) argv.push(a.value)
      }
      const spec = firstPackage.version
        ? `${firstPackage.identifier}@${firstPackage.version}`
        : firstPackage.identifier
      argv.push(spec)
      for (const a of firstPackage.packageArguments ?? []) {
        if (a.value) argv.push(a.value)
      }
      args = argv
      for (const ev of firstPackage.environmentVariables ?? []) {
        envEntries.push({
          key: ev.name,
          value: String(ev.value ?? ev.default ?? ''),
          is_secret: !!ev.isSecret,
        })
      }
    }

    return {
      fields: {
        name: slug,
        display_name: displayTitle,
        description: server.description,
        transport_type: transport,
        command,
        args,
        url,
        environment_variables_entries: envEntries,
        headers_entries: headerEntries,
        supports_sampling: false,
        enabled: true,
      },
      hub_id: server.name,
    }
  }

  const handleInstall = () => {
    try {
      setInstalling(true)
      Stores.McpServerDrawer.openMcpServerDrawer(
        undefined,
        'create',
        prefillFromHub(),
      )
      message.info('Review settings and configure any required secrets, then save.')
    } finally {
      setInstalling(false)
    }
  }

  const handleInstallAsSystem = () => {
    try {
      setInstallingSystem(true)
      Stores.McpServerDrawer.openMcpServerDrawer(
        undefined,
        'create-system',
        prefillFromHub(),
      )
      message.info('Review settings and configure any required secrets, then save.')
    } finally {
      setInstallingSystem(false)
    }
  }

  const repoUrl = server.repository?.url
  const homepageUrl = server.websiteUrl

  return (
    <>
      <Card
        hoverable
        className="cursor-pointer relative group hover:!shadow-md transition-shadow h-full"
        onClick={() => setShowDetails(true)}
        data-server-id={server.name}
        data-testid={`hub-mcp-card-${server.name}`}
      >
        <div className="flex items-start gap-3 flex-wrap">
          <div className="flex-1">
            <div className="flex items-center gap-2 mb-2 flex-wrap">
              <div className="flex-1 min-w-48">
                <Flex className="gap-2 items-center">
                  <Text className="font-medium cursor-pointer">{displayTitle}</Text>
                  {server.version && (
                    <Tag className="text-xs !m-0">v{server.version}</Tag>
                  )}
                  {/* Provenance badge: ingested MCP registry entries
                      carry `_meta["io.modelcontextprotocol.registry"]`. */}
                  {!!(
                    server._meta &&
                    (server._meta as Record<string, unknown>)[
                      'io.modelcontextprotocol.registry'
                    ]
                  ) && (
                    <Tooltip content="From the official Model Context Protocol registry">
                      <Tag tone="info" className="text-xs !m-0">
                        MCP Registry
                      </Tag>
                    </Tooltip>
                  )}
                  <Tag className="text-xs">{transportLabel}</Tag>
                  {installing && <Tag tone="info">Installing...</Tag>}
                  {isAlreadyInstalled && <Tag tone="success">Installed</Tag>}
                  {isAlreadyInstalledAsSystem && (
                    <Tag tone="info">System installed</Tag>
                  )}
                </Flex>
              </div>
              <div className="flex gap-1 items-center justify-end">
                {homepageUrl && (
                  <Button
                    icon={<GlobalOutlined />}
                    onClick={e => {
                      e.stopPropagation()
                      window.open(homepageUrl, '_blank')
                    }}
                  />
                )}
                {repoUrl && (
                  <Button
                    icon={<GithubOutlined />}
                    onClick={e => {
                      e.stopPropagation()
                      window.open(repoUrl, '_blank')
                    }}
                  />
                )}
                {multiUserMode &&
                  (isAlreadyInstalled ? (
                    <Button
                      icon={<EyeOutlined />}
                      onClick={e => {
                        e.stopPropagation()
                        navigate('/settings/mcp-servers')
                      }}
                      data-testid="hub-mcp-view-btn"
                    >
                      View Server
                    </Button>
                  ) : canInstall ? (
                    <Button
                      icon={<DownloadOutlined />}
                      onClick={e => {
                        e.stopPropagation()
                        handleInstall()
                      }}
                      disabled={installing || installingSystem}
                      loading={installing}
                      data-testid="hub-mcp-install-btn"
                    >
                      {canInstallSystem ? 'Install for me' : 'Install'}
                    </Button>
                  ) : null)}
                {canInstallSystem && (
                  <Button
                    icon={<CopyOutlined />}
                    onClick={e => {
                      e.stopPropagation()
                      handleInstallAsSystem()
                    }}
                    loading={installingSystem}
                    disabled={
                      installing ||
                      installingSystem ||
                      isAlreadyInstalledAsSystem
                    }
                    data-testid="hub-mcp-install-as-system-btn"
                  >
                    {isAlreadyInstalledAsSystem
                      ? 'System Installed'
                      : 'Install for the system'}
                  </Button>
                )}
              </div>
            </div>

            <div>
              {server.description && (
                <Text type="secondary" className="text-sm mb-2 block">
                  {server.description}
                </Text>
              )}
            </div>
          </div>
        </div>
      </Card>

      <McpServerDetailsDrawer
        server={server}
        open={showDetails}
        onClose={() => setShowDetails(false)}
      />
    </>
  )
}
