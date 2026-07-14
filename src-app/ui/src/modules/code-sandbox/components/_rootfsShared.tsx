import { Button, Tooltip } from '@ziee/kit'
import {
  Permissions,
  type DrainEntry,
  type InstallTaskState,
  type RootfsArtifact,
  type RootfsRelease,
} from '@/api-client/types'

// ---------------------------------------------------------------------------
// Shared constants + pure helpers + view-model for the rootfs-versions section.
//
// The section reads store state at its top (hook-safety) and builds the
// version-grouped view-model with `buildVersionGroups`; the card / group
// children receive the pre-built groups + callbacks as props and never read
// the store. Keeping all the logic here (a) keeps each component's hook
// surface small and (b) lets the grouping be reasoned about in isolation.
// ---------------------------------------------------------------------------

export const MANAGE_PERM = Permissions.CodeSandboxEnvironmentsManage
export const READ_PERM = Permissions.CodeSandboxEnvironmentsRead

const DEFAULT_ARCH = 'x86_64'
const DEFAULT_FLAVORS = ['minimal', 'full']
const DEFAULT_PACKAGE = 'squashfs'

/** Known host architectures, longest-first so prefix matching is unambiguous. */
const KNOWN_ARCHES = ['aarch64', 'x86_64']
const ASSET_PREFIX = 'ziee-sandbox-rootfs-'

/** Per-(version,arch,flavor,package) action flags mirrored from the store
 *  (the store does not export its `ActionState` type). */
export interface ActionFlags {
  installing?: boolean
  pinning?: boolean
  deleting?: boolean
}

export interface FlavorEntry {
  flavor: string
  arch: string
  pkg: string
  /** `${version}::${arch}::${flavor}::${pkg}` — matches the store's keys. */
  rowKey: string
  /** Present iff this flavor is downloaded. */
  artifact?: RootfsArtifact
  /** Present in the GitHub release catalog (so it can be downloaded). */
  available: boolean
  /** Live install task (from `installTasks[rowKey]`). */
  task?: InstallTaskState
  /** Live drain entry (from `draining`, keyed by version::arch::flavor). */
  drainEntry?: DrainEntry
  isInstalling: boolean
  /** inflight exec + MCP for this flavor. */
  live: number
  isDraining: boolean
}

export interface VersionGroup {
  version: string
  /** Downloaded flavors first, then alphabetical. */
  flavors: FlavorEntry[]
  isDefault: boolean
  /** ≥1 flavor downloaded (any arch). */
  anyDownloaded: boolean
  /** No host-arch catalog flavor is missing an artifact. */
  allDownloaded: boolean
  /** Host-arch catalog flavors with no artifact yet — drives Download-all. */
  missingFlavors: FlavorEntry[]
  release?: RootfsRelease
}

function parseSemver(v: string): [number, number, number] {
  const parts = v.split('.').map(p => parseInt(p, 10) || 0)
  return [parts[0] ?? 0, parts[1] ?? 0, parts[2] ?? 0]
}

export function isMajorBump(oldV: string | null, newV: string): boolean {
  if (!oldV) return false
  return parseSemver(oldV)[0] !== parseSemver(newV)[0]
}

// Install phases come from the backend's `InstallProgress` enum; map each one
// to a coarse stepped percentage (the backend emits discrete phases, not
// byte-granular progress).
export function phasePercent(phase?: string | null): number {
  switch (phase) {
    case 'resolving':
      return 10
    case 'downloading':
      return 50
    case 'verifying_sha256':
      return 75
    case 'verifying_cosign':
      return 85
    case 'installing':
      return 95
    case 'complete':
      return 100
    // A failed flavor fills the bar so the antd `exception` status renders a
    // solid red bar (reads as "failed", not a ~5% "barely started" sliver).
    case 'failed':
      return 100
    default:
      return 5
  }
}

/**
 * Parse a release asset filename
 * (`ziee-sandbox-rootfs-{arch}-{flavor}.{squashfs|tar.zst}`) into its parts.
 * Returns null for names that don't match the shape. Arch is matched against
 * the known set first (arch tokens never contain `-`), so the remainder is
 * unambiguously the flavor.
 */
function parseAssetName(
  name: string,
): { arch: string; flavor: string; pkg: string } | null {
  if (!name.startsWith(ASSET_PREFIX)) return null
  let rest = name.slice(ASSET_PREFIX.length)

  let pkg: string
  if (rest.endsWith('.tar.zst')) {
    pkg = 'tar.zst'
    rest = rest.slice(0, -'.tar.zst'.length)
  } else if (rest.endsWith('.squashfs')) {
    pkg = 'squashfs'
    rest = rest.slice(0, -'.squashfs'.length)
  } else {
    return null
  }

  for (const arch of KNOWN_ARCHES) {
    if (rest.startsWith(`${arch}-`)) {
      const flavor = rest.slice(arch.length + 1)
      if (flavor) return { arch, flavor, pkg }
    }
  }
  // Fallback: split on the first dash (arch can't contain one).
  const dash = rest.indexOf('-')
  if (dash > 0 && dash < rest.length - 1) {
    return { arch: rest.slice(0, dash), flavor: rest.slice(dash + 1), pkg }
  }
  return null
}

/** Derive the host arch from installed artifacts (operators only download for
 *  their own host), defaulting to x86_64. No host-arch endpoint exists yet; a
 *  future one can replace this derivation cleanly. */
export function deriveHostArch(installed: RootfsArtifact[]): string {
  const counts = new Map<string, number>()
  for (const a of installed) {
    counts.set(a.arch, (counts.get(a.arch) ?? 0) + 1)
  }
  // Seed with the default's own count so a tie deterministically keeps the
  // default (only a strictly larger count displaces it) regardless of Map order.
  let best = DEFAULT_ARCH
  let bestN = counts.get(DEFAULT_ARCH) ?? 0
  for (const [arch, n] of counts) {
    if (n > bestN) {
      best = arch
      bestN = n
    }
  }
  return best
}

/** Derive the host's package type (squashfs on Linux/macOS, tar.zst on Windows
 *  WSL) from installed artifacts, defaulting to squashfs. A release ships BOTH
 *  packages per flavor, so this is used to collapse the catalog to one row per
 *  flavor. Same "no host endpoint yet" caveat as deriveHostArch. */
export function deriveHostPackage(installed: RootfsArtifact[]): string {
  const counts = new Map<string, number>()
  for (const a of installed) {
    counts.set(a.package, (counts.get(a.package) ?? 0) + 1)
  }
  // Seed with the default's count so ties deterministically keep the default.
  let best = DEFAULT_PACKAGE
  let bestN = counts.get(DEFAULT_PACKAGE) ?? 0
  for (const [pkg, n] of counts) {
    if (n > bestN) {
      best = pkg
      bestN = n
    }
  }
  return best
}

/** Lower is better: the host package wins, then squashfs, then tar.zst. Used to
 *  pick ONE package per (version, flavor) when a release publishes several. */
function packagePriority(pkg: string, hostPkg: string): number {
  if (pkg === hostPkg) return 0
  if (pkg === 'squashfs') return 1
  if (pkg === 'tar.zst') return 2
  return 3
}

function rowKeyOf(version: string, arch: string, flavor: string, pkg: string) {
  return `${version}::${arch}::${flavor}::${pkg}`
}

interface BuildArgs {
  installed: RootfsArtifact[]
  available: RootfsRelease[]
  hostArch: string
  hostPkg: string
  pinnedVersion: string | null
  installTasks: Record<string, InstallTaskState>
  actions: Record<string, ActionFlags>
  draining: DrainEntry[]
}

/**
 * Fold installed artifacts + the GitHub release catalog into version groups,
 * each carrying its flavor sub-entries (with live task / drain state attached).
 * Sorted newest-version first.
 */
export function buildVersionGroups({
  installed,
  available,
  hostArch,
  hostPkg,
  pinnedVersion,
  installTasks,
  actions,
  draining,
}: BuildArgs): VersionGroup[] {
  const drainByKey = new Map<string, DrainEntry>()
  for (const d of draining) {
    drainByKey.set(`${d.version}::${d.arch}::${d.flavor}`, d)
  }

  interface Acc {
    version: string
    // Keyed by `arch::flavor` — package-AGNOSTIC, so an installed artifact and
    // a catalog asset for the same flavor (even in DIFFERENT packages) collapse
    // to ONE row. The concrete package is resolved per-entry in the finalize
    // pass (artifact's package wins, else the host-preferred catalog package).
    byKey: Map<string, FlavorEntry>
    // Catalog packages seen per `arch::flavor`, used to pick the host's package
    // for a not-yet-downloaded flavor.
    catalogPkgs: Map<string, string[]>
    release?: RootfsRelease
  }
  const groups = new Map<string, Acc>()

  const ensure = (version: string): Acc => {
    let g = groups.get(version)
    if (!g) {
      g = { version, byKey: new Map(), catalogPkgs: new Map() }
      groups.set(version, g)
    }
    return g
  }

  const ensureFlavor = (g: Acc, arch: string, flavor: string): FlavorEntry => {
    const key = `${arch}::${flavor}`
    let f = g.byKey.get(key)
    if (!f) {
      f = {
        flavor,
        arch,
        pkg: hostPkg, // placeholder; resolved in the finalize pass below
        rowKey: '',
        available: false,
        isInstalling: false,
        live: 0,
        isDraining: false,
      }
      g.byKey.set(key, f)
    }
    return f
  }

  // 1. Downloaded artifacts (any arch). The artifact's own package is
  //    authoritative for that flavor's row. Non-host-arch artifacts still
  //    surface as flavor sub-rows, but the version-level Delete only appears on
  //    a fully-downloaded host-arch version in the Downloaded card — a stray
  //    non-host-arch artifact for a version still in the catalog has no UI
  //    delete affordance today.
  for (const a of installed) {
    const f = ensureFlavor(ensure(a.version), a.arch, a.flavor)
    f.artifact = a
  }

  // 2. GitHub catalog → the downloadable host-arch flavors per version. A
  //    release publishes BOTH a .squashfs (Linux/macOS) and a .tar.zst
  //    (Windows) asset per flavor; record every package but keep ONE row per
  //    flavor (package chosen in the finalize pass) so a flavor never appears
  //    twice.
  for (const r of available) {
    if (r.draft || r.prerelease) continue
    const g = ensure(r.version)
    g.release = r

    const parsed = (r.asset_names ?? [])
      .map(parseAssetName)
      .filter((p): p is { arch: string; flavor: string; pkg: string } => !!p)
      .filter(p => p.arch === hostArch)

    // No parseable host-arch assets → fall back to the conventional flavors.
    const tuples =
      parsed.length > 0
        ? parsed
        : DEFAULT_FLAVORS.map(flavor => ({ arch: hostArch, flavor, pkg: hostPkg }))

    for (const { arch, flavor, pkg } of tuples) {
      const f = ensureFlavor(g, arch, flavor)
      f.available = true
      const key = `${arch}::${flavor}`
      const pkgs = g.catalogPkgs.get(key) ?? []
      if (!pkgs.includes(pkg)) pkgs.push(pkg)
      g.catalogPkgs.set(key, pkgs)
    }
  }

  // 3. Resolve each flavor's package + rowKey, attach live task/drain state,
  //    and finalise group-level flags.
  const out: VersionGroup[] = []
  for (const g of groups.values()) {
    const flavors = Array.from(g.byKey.values())
    for (const f of flavors) {
      // Package precedence: the downloaded artifact's package wins; else the
      // host's package among the catalog's offerings (else squashfs, else
      // whatever's published, else the host default).
      if (f.artifact) {
        f.pkg = f.artifact.package
      } else {
        const cands = g.catalogPkgs.get(`${f.arch}::${f.flavor}`) ?? []
        f.pkg = cands.length
          ? cands.reduce((best, p) =>
              packagePriority(p, hostPkg) < packagePriority(best, hostPkg) ? p : best,
            )
          : hostPkg
      }
      f.rowKey = rowKeyOf(g.version, f.arch, f.flavor, f.pkg)
      f.task = installTasks[f.rowKey]
      f.drainEntry = drainByKey.get(`${g.version}::${f.arch}::${f.flavor}`)
      f.isInstalling =
        !!actions[f.rowKey]?.installing || f.task?.status === 'running'
      f.live =
        (f.drainEntry?.inflight_exec ?? 0) + (f.drainEntry?.inflight_mcp ?? 0)
      f.isDraining = !!f.drainEntry && g.version !== pinnedVersion && f.live > 0
    }

    flavors.sort((a, b) => {
      const ad = a.artifact ? 0 : 1
      const bd = b.artifact ? 0 : 1
      if (ad !== bd) return ad - bd
      return a.flavor.localeCompare(b.flavor)
    })

    const hostCatalog = flavors.filter(f => f.arch === hostArch && f.available)
    const missingFlavors = hostCatalog.filter(f => !f.artifact)
    const anyDownloaded = flavors.some(f => !!f.artifact)

    out.push({
      version: g.version,
      flavors,
      release: g.release,
      isDefault: g.version === pinnedVersion,
      anyDownloaded,
      allDownloaded: anyDownloaded && missingFlavors.length === 0,
      missingFlavors,
    })
  }

  out.sort((a, b) => {
    const av = parseSemver(a.version)
    const bv = parseSemver(b.version)
    for (let i = 0; i < 3; i++) {
      if (bv[i] !== av[i]) return bv[i] - av[i]
    }
    return 0
  })
  return out
}

// ---------------------------------------------------------------------------
// Shared permission-gated button. Mirrors the original section's helper:
// visible-but-disabled with a tooltip naming the required permission when the
// viewer lacks `code_sandbox::environments::manage` (the E2E suite asserts the
// buttons are `.toBeDisabled()`, not hidden — so we do NOT use <Can>).
// ---------------------------------------------------------------------------

interface RenderButtonProps {
  canManage: boolean
  label: string
  icon: React.ReactNode
  onClick: () => void
  loading?: boolean
  danger?: boolean
  /** When set, the button is disabled regardless of permission and wrapped in a
   *  tooltip explaining why (e.g. the sandbox isn't initialized, so installing
   *  would 503). Takes precedence over the requires-manage tooltip. */
  disabledReason?: string
  'data-testid': string
}

export function RenderButton({
  canManage,
  label,
  icon,
  onClick,
  loading,
  danger,
  disabledReason,
  'data-testid': testId,
}: RenderButtonProps) {
  const btn = (
    <Button
      variant={danger ? 'destructive' : 'ghost'}
      icon={icon}
      loading={loading}
      disabled={!canManage || loading || !!disabledReason}
      onClick={onClick}
      data-testid={testId}
    >
      {label}
    </Button>
  )
  if (disabledReason) {
    return <Tooltip content={disabledReason}>{btn}</Tooltip>
  }
  return canManage ? (
    btn
  ) : (
    <Tooltip content={`Requires ${MANAGE_PERM}`}>{btn}</Tooltip>
  )
}
