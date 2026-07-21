/**
 * Dev-gallery seed for the `hardware` module — the HardwareMonitor page: no-GPU
 * empty card, live-metrics (SSE-seeded) shadow, and cold-load error. Auto-
 * discovered by the gallery's runtime registry (`@/dev/gallery/support`); never
 * imported by `module.tsx`, so it is dev-only and tree-shaken from prod.
 */
import type { ModuleGallery } from '@/dev/gallery/support'
import { holdPatch, lazyNamed } from '@/dev/gallery/support'

export const gallery: ModuleGallery = {
  seeded: [
    // ── HardwareMonitor: no GPU devices → the "GPU Usage" empty card. currentUsage
    // arrives via the live hardware SSE (not a GET), so seed it on the store. ─────
    {
      slug: 'seeded-hardware-no-gpu',
      title: 'Hardware monitor — no GPU',
      note: '!currentUsage.gpu_devices.length → the GPU-empty card',
      path: '/',
      initialPath: '/',
      component: lazyNamed(
        () => import('@/modules/hardware/HardwareMonitor'),
        'HardwareMonitor',
      ),
      setup: async () => {
        const { HardwareDef } = await import('@/modules/hardware/hardware')
        await holdPatch(() =>
          HardwareDef.store.setState({
            currentUsage: {
              cpu: { usage_percentage: 12 },
              memory: {
                available_ram: 8_000_000_000,
                used_ram: 8_000_000_000,
                usage_percentage: 50,
              },
              gpu_devices: [],
              timestamp: new Date().toISOString(),
            },
            usageLoading: false,
            usageError: null,
          } as any),
        )
      },
    },
    // ── SHADOW: /hardware-monitor live metrics. Usage data arrives over the
    // `/api/hardware/stream` SSE connection, not a JSON GET, so the GET-only loaded
    // cassette leaves `currentUsage` null → "Waiting for usage data…". Seed a
    // realistic snapshot (CPU/mem/GPU) so the charts render. ──────────────────────
    {
      slug: 'hardware-monitor',
      title: 'Hardware monitor — live metrics',
      note: 'currentUsage arrives over SSE (not a JSON GET); seed a realistic usage snapshot so the CPU/memory/GPU charts render instead of "Waiting for usage data…".',
      path: '/hardware-monitor',
      initialPath: '/hardware-monitor',
      component: lazyNamed(
        () => import('@/modules/hardware/HardwareMonitor'),
        'HardwareMonitor',
      ),
      setup: async () => {
        const { HardwareDef } = await import('@/modules/hardware/hardware')
        await holdPatch(() =>
          HardwareDef.store.setState({
            hardwareInfo: {
              cpu: {
                architecture: 'x86_64',
                model: 'AMD Ryzen 9 7950X',
                cores: 16,
                threads: 32,
                base_frequency: 4500,
                max_frequency: 5700,
              },
              gpu_devices: [
                {
                  device_id: 'gpu-0',
                  name: 'NVIDIA GeForce RTX 4090',
                  vendor: 'NVIDIA',
                  memory: 25757220864,
                  driver_version: '550.90.07',
                  compute_capabilities: {} as any,
                },
              ],
              memory: { total_ram: 68719476736, total_swap: 8589934592 },
              operating_system: {
                architecture: 'x86_64',
                name: 'Linux',
                version: '24.04',
                kernel_version: '6.8.0',
              },
            },
            hardwareInitialized: true,
            hardwareLoading: false,
            hardwareError: null,
            currentUsage: {
              cpu: { usage_percentage: 37.4, temperature: 58, frequency: 4820 },
              gpu_devices: [
                {
                  device_id: 'gpu-0',
                  device_name: 'NVIDIA GeForce RTX 4090',
                  utilization_percentage: 72,
                  memory_total: 25757220864,
                  memory_used: 14200000000,
                  memory_usage_percentage: 55.1,
                  temperature: 64,
                  power_usage: 285,
                },
              ],
              memory: {
                total_ram: 68719476736,
                used_ram: 28051503104,
                available_ram: 40667973632,
                usage_percentage: 40.8,
                used_swap: 0,
                available_swap: 8589934592,
              } as any,
              timestamp: new Date().toISOString(),
            },
            usageLoading: false,
            usageError: null,
            sseConnected: true,
            sseConnecting: false,
            sseError: null,
          } as any),
        )
      },
    },
    // ── /hardware-monitor cold-load ERROR. The metrics shadow above seeds a full
    // snapshot so it can only ever show the loaded charts; the error branch
    // (`hardwareError && !hardwareInfo` → ErrorState) is otherwise unreachable in
    // the gallery because the shadow owns the `hardware-monitor` slug. Seed a
    // load failure (no hardwareInfo) so the real ErrorState is reviewable. ───────
    {
      slug: 'seeded-hardware-monitor-error',
      title: 'Hardware monitor — load error',
      note: 'hardwareError && !hardwareInfo → the in-place "Couldn\'t load hardware monitor" ErrorState (the cold hardware-info GET failed).',
      path: '/hardware-monitor',
      initialPath: '/hardware-monitor',
      component: lazyNamed(
        () => import('@/modules/hardware/HardwareMonitor'),
        'HardwareMonitor',
      ),
      setup: async () => {
        const { HardwareDef } = await import('@/modules/hardware/hardware')
        // holdPatch re-asserts the failure so the store's init loadHardwareInfo()
        // (which succeeds against the loaded cassette) can't clobber it back to a
        // healthy state.
        await holdPatch(() =>
          HardwareDef.store.setState({
            hardwareInfo: null,
            hardwareInitialized: false,
            hardwareLoading: false,
            hardwareError: 'Internal server error',
            currentUsage: null,
            usageLoading: false,
            sseConnected: false,
            sseConnecting: false,
          } as any),
        )
      },
    },
  ],
}
