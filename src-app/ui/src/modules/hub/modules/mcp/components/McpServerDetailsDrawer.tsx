import type { ReactNode } from 'react'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { Flex, Tag, Card, Text, Title } from '@ziee/kit'
import { Link } from 'lucide-react'
import type { HubMCPServer } from '@/api-client/types'
import { Stores } from '@ziee/framework/stores'

interface McpServerDetailsDrawerProps {
  server: HubMCPServer | null
  open: boolean
  onClose: () => void
  /** Install actions (mirrors the card) rendered in the drawer footer. */
  footer?: ReactNode
}

/**
 * Strict server.json drawer. v1 surfaced display_name / category /
 * transport_type / command / args / url / headers / author / homepage /
 * popularity_score / required_env / required_headers; all of those
 * dropped with the move to the official server.json shape. Display
 * now drives off `name` (reverse-DNS), `description`, `version`,
 * `repository.url`, `websiteUrl`, and the first `packages[]` or
 * `remotes[]` entry to show the install command / URL.
 */
export function McpServerDetailsDrawer({
  server,
  open,
  onClose,
  footer,
}: McpServerDetailsDrawerProps) {
  if (!server) return null

  // Display title: prefer the curated `IndexItem.title` (publisher
  // sets via `_hub_curation.title` in the source YAML); fall back to
  // the leaf of the reverse-DNS name. The full reverse-DNS lives
  // below as a subtitle so operators can see the upstream identity.
  const leaf = (() => {
    const slash = server.name.indexOf('/')
    return slash >= 0 ? server.name.slice(slash + 1) : server.name
  })()
  const indexItem = Stores.HubCatalog.catalog?.items?.find(
    it => it.category === 'mcp-server' && it.name === server.name,
  )
  const displayTitle = indexItem?.title ?? leaf

  // Resolve the install surface: prefer the first remote (http/sse),
  // fall back to the first package (stdio).
  const firstRemote = server.remotes && server.remotes[0]
  const firstPackage = server.packages && server.packages[0]

  return (
    <Drawer title={displayTitle} open={open} onClose={onClose} footer={footer}>
      <Flex vertical className="gap-4" data-testid="hub-mcp-detail-sheet">
        {/* Basic Info */}
        <div>
          <Title level={3} className="!m-0 !mb-2">
            {displayTitle}
          </Title>
          <Text type="secondary" className="text-xs break-all">
            {server.name}
            {server.version ? ` · v${server.version}` : ''}
          </Text>
          {server.description && (
            <div className="mt-2">
              <Text type="secondary">{server.description}</Text>
            </div>
          )}
        </div>

        {/* Connection — remote URL (http/sse) takes priority; falls
            back to the package command (stdio). */}
        {firstRemote ? (
          <div>
            <Title level={5}>Remote endpoint</Title>
            <Card size="sm" className="bg-muted" data-testid="hub-mcp-detail-remote-card">
              <a
                href={firstRemote.url}
                target="_blank"
                rel="noopener noreferrer"
                className="inline-flex items-center gap-1 text-xs break-all"
              >
                <Link /> {firstRemote.url}
              </a>
              <div className="mt-1">
                <Tag tone="info" className="text-xs" data-testid="hub-mcp-detail-remote-type-tag">
                  {firstRemote.type ?? 'remote'}
                </Tag>
              </div>
            </Card>
          </div>
        ) : firstPackage ? (
          <div>
            <Title level={5}>Install command</Title>
            <Card size="sm" className="bg-muted" data-testid="hub-mcp-detail-install-card">
              <Text code className="text-xs break-all">
                {firstPackage.runtimeHint ?? 'run'}{' '}
                {(firstPackage.runtimeArguments ?? [])
                  .map(a => a.value)
                  .filter(Boolean)
                  .join(' ')}{' '}
                {firstPackage.identifier}@{firstPackage.version}{' '}
                {(firstPackage.packageArguments ?? [])
                  .map(a => a.value)
                  .filter(Boolean)
                  .join(' ')}
              </Text>
              <div className="mt-1">
                <Tag tone="info" className="text-xs" data-testid="hub-mcp-detail-install-tag">
                  {firstPackage.registryType} · {firstPackage.transport?.type ?? 'stdio'}
                </Tag>
              </div>
            </Card>
          </div>
        ) : (
          <div>
            <Title level={5}>Connection</Title>
            <Text type="secondary" className="text-xs">
              No packages or remotes declared in the manifest.
            </Text>
          </div>
        )}

        {/* Env vars declared on the first package — these get seeded
            into the installed server's env map; the user fills in
            their tokens post-install. */}
        {firstPackage?.environmentVariables &&
          firstPackage.environmentVariables.length > 0 && (
            <div>
              <Title level={5}>Environment variables</Title>
              <Card size="sm" data-testid="hub-mcp-detail-env-card">
                <Flex vertical className="gap-1">
                  {firstPackage.environmentVariables.map(ev => (
                    <Flex
                      key={ev.name}
                      className="flex justify-between text-xs"
                    >
                      <Text code>{ev.name}</Text>
                      {ev.isSecret && (
                        <Tag tone="warning" className="text-xs" data-testid={`hub-mcp-detail-env-secret-tag-${ev.name}`}>
                          secret
                        </Tag>
                      )}
                    </Flex>
                  ))}
                </Flex>
              </Card>
            </div>
          )}

        {/* Headers declared on the first remote. */}
        {firstRemote?.headers && firstRemote.headers.length > 0 && (
          <div>
            <Title level={5}>Headers</Title>
            <Card size="sm" data-testid="hub-mcp-detail-headers-card">
              <Flex vertical className="gap-1">
                {firstRemote.headers.map(h => (
                  <Flex
                    key={h.name}
                    className="flex justify-between text-xs"
                  >
                    <Text code>{h.name}</Text>
                    {h.isSecret && (
                      <Tag tone="warning" className="text-xs" data-testid={`hub-mcp-detail-header-secret-tag-${h.name}`}>
                        secret
                      </Tag>
                    )}
                  </Flex>
                ))}
              </Flex>
            </Card>
          </div>
        )}

        {/* Links */}
        {(server.repository?.url || server.websiteUrl) && (
          <div>
            <Title level={5}>Links</Title>
            <Flex vertical className="gap-2">
              {server.repository?.url && (
                <a
                  href={server.repository.url}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="inline-flex items-center gap-2"
                >
                  <Link /> Repository
                </a>
              )}
              {server.websiteUrl && (
                <a
                  href={server.websiteUrl}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="inline-flex items-center gap-2"
                >
                  <Link /> Website
                </a>
              )}
            </Flex>
          </div>
        )}
      </Flex>
    </Drawer>
  )
}
