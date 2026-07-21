import { ApiClient } from '@/api-client'
import type { RuntimeConfigGet, RuntimeConfigSet } from '../state'

export default (set: RuntimeConfigSet, _get: RuntimeConfigGet) =>
  async () => {
    set(s => {
      s.loadingGpu = true
    })
    // detect-gpu spawns host probes and can transiently 502 on a cold backend;
    // retry a few times with backoff before giving up so the card isn't blank.
    const delays = [1000, 2000, 3000]
    for (let attempt = 0; attempt <= delays.length; attempt++) {
      try {
        const gpu = await ApiClient.LocalRuntime.detectGpu(undefined)
        set(s => {
          s.gpu = gpu
          s.loadingGpu = false
        })
        return
      } catch (error) {
        if (attempt === delays.length) {
          set(s => {
            s.error = error instanceof Error ? error.message : 'GPU detection failed'
            s.loadingGpu = false
          })
        } else {
          await new Promise(r => setTimeout(r, delays[attempt]))
        }
      }
    }
  }
