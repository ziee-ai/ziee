/**
 * Dev-gallery seed for the `memory` module — core-memory block editor states +
 * the memory audit-log grid. Auto-discovered by the gallery's runtime registry
 * (`@/dev/gallery/support`); never imported by `module.tsx`, so it is dev-only
 * and tree-shaken from prod.
 */
import type { ModuleGallery } from '@/dev/gallery/support'
import { holdPatch, lazyNamed, lazyProps } from '@/dev/gallery/support'

export const gallery: ModuleGallery = {
  seeded: [
    // ── CoreMemoryBlocksEditor: loading / empty (prop assistantId). ──────────────
    {
      slug: 'seeded-core-memory-loading',
      title: 'Core memory blocks — loading',
      note: 'blocks empty && loading → the load spinner',
      path: '/',
      initialPath: '/',
      component: lazyProps(
        () => import('@/modules/memory/components/CoreMemoryBlocksEditor'),
        'CoreMemoryBlocksEditor',
        { assistantId: 'asst-1' },
      ),
      setup: async () => {
        const { useCoreMemoryBlocksStore } = await import(
          '@/modules/memory/stores/coreMemoryBlocks'
        )
        await holdPatch(() =>
          useCoreMemoryBlocksStore.setState({
            blocksByAssistant: { 'asst-1': [] },
            loadingByAssistant: { 'asst-1': true },
          } as any),
        )
      },
    },
    {
      slug: 'seeded-core-memory-empty',
      title: 'Core memory blocks — empty',
      note: 'blocks empty && !loading → "No blocks yet" empty',
      path: '/',
      initialPath: '/',
      component: lazyProps(
        () => import('@/modules/memory/components/CoreMemoryBlocksEditor'),
        'CoreMemoryBlocksEditor',
        { assistantId: 'asst-1' },
      ),
      setup: async () => {
        const { useCoreMemoryBlocksStore } = await import(
          '@/modules/memory/stores/coreMemoryBlocks'
        )
        await holdPatch(() =>
          useCoreMemoryBlocksStore.setState({
            blocksByAssistant: { 'asst-1': [] },
            loadingByAssistant: { 'asst-1': false },
          } as any),
        )
      },
    },
    // ── AuditLogSection: LOADED with memory-audit rows (kit-Table sort/filter). ──
    {
      slug: 'seeded-memory-audit-loaded',
      title: 'Memory audit log — loaded',
      note: 'Stores.MemoryAudit.entries → sortable/filterable grid rows',
      path: '/',
      initialPath: '/',
      component: lazyNamed(
        () => import('@/modules/memory/components/sections/AuditLogSection'),
        'AuditLogSection',
      ),
      setup: async () => {
        const { useMemoryAuditStore } = await import(
          '@/modules/memory/stores/memoryAudit'
        )
        const mk = (
          id: number,
          op: string,
          source: string,
          snapshot: string,
        ) => ({
          id,
          op,
          source,
          actor_kind: 'user',
          content_snapshot: snapshot,
          created_at: `2026-01-0${id}T10:00:00Z`,
          metadata: {},
          user_id: 'u-1',
        })
        await holdPatch(() =>
          useMemoryAuditStore.setState({
            entries: [
              mk(1, 'ADD', 'manual', 'Likes espresso'),
              mk(2, 'UPDATE', 'extraction', 'Works at Acme'),
              mk(3, 'DELETE', 'mcp_tool', 'Old preference'),
            ],
            loading: false,
            limit: 100,
            error: null,
          } as never),
        )
      },
    },
  ],
}
