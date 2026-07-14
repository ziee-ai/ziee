import { chatExtensionRegistry } from '@/modules/chat/core/extensions'
import type { ChatExtStoreApi } from '@/modules/chat/core/extensions/types'
import type { StoreProxy } from '@/core/stores'

/**
 * Per-pane extension runtime (ITEM-34). The `chatExtensionRegistry` is the global
 * CATALOG — the registration descriptors (extensions / slots / content-types /
 * SSE handlers / delta+content providers) + the stateless dispatch/render methods
 * (which already thread the calling store's `get`/`set`, so they route per-pane).
 * This runtime is the *per-pane lifecycle* half the catalog used to own as a
 * singleton: it holds THIS pane's `initialized` flag + the pane's chat store api +
 * a resolver for the pane's own extension-store instances, and runs
 * initialize/cleanup against them.
 *
 * Why a per-pane flag: the catalog's single `initialized` made the SECOND pane's
 * `initialize()` early-return "already initialized" — silently skipping its
 * extension subscriptions + keyboard/file/edit wiring — and any one pane's
 * unmount `cleanup()` flipped the shared flag, disarming the survivors (GAP-1).
 * Each pane owning its own flag + running initialize/cleanup with its own `ctx`
 * fixes both. Single-pane keeps calling the catalog's gated `initialize()`
 * directly (no runtime), so it is byte-identical.
 */
export class PaneExtensionRuntime {
  private initialized = false

  constructor(
    private readonly chatStoreApi: ChatExtStoreApi,
    private readonly resolveStore: (name: string) => StoreProxy<any> | null,
  ) {}

  /** Seed a fresh per-pane instance of every extension store into the pane's
   *  chat state (idempotent — skips already-present names). */
  injectExtensionStores(chatState: Record<string, unknown>): void {
    chatExtensionRegistry.injectExtensionStores(chatState)
  }

  async initialize(): Promise<void> {
    if (this.initialized) return
    await chatExtensionRegistry.initializeExtensions(
      this.chatStoreApi,
      this.resolveStore,
    )
    this.initialized = true
  }

  async cleanup(): Promise<void> {
    if (!this.initialized) return
    await chatExtensionRegistry.cleanupExtensions(
      this.chatStoreApi,
      this.resolveStore,
    )
    this.initialized = false
  }
}
