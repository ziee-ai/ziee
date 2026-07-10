import type { Page, Route } from '@playwright/test'
import type {
  AvailableUpdatesResponse2,
  DownloadSnapshot2,
  RuntimeVersionResponse2,
  TranscriptionResponse,
  VoiceCapability,
  VoiceInstanceInfo,
  VoiceModelStatus,
  VoiceSettings,
} from '../../../src/api-client/types'

/**
 * Shared harness for the 14-voice E2E suite.
 *
 * The voice feature is capability-driven and talks to `/api/voice/**`; the
 * composer mic also needs a real browser MediaRecorder + a secure context. In
 * E2E we make BOTH deterministic:
 *
 *   1. `installVoiceBrowserMocks` — an addInitScript that forces
 *      `window.isSecureContext`, a fake `navigator.mediaDevices.getUserMedia`
 *      (grant / deny), a minimal fake `MediaRecorder` that emits a REAL tiny
 *      16 kHz mono WAV blob (so the store's `recordedBlobToWav16k` →
 *      `AudioContext.decodeAudioData` round-trips for real), and `MediaRecorder`
 *      on `window`. A `recording:false` variant removes the capture APIs so the
 *      mic HIDES (the "unsupported browser" posture).
 *   2. `routeVoice` — a single `page.route('**\/api/voice/**')` that fulfils
 *      every voice endpoint from a mutable in-memory state, so a spec can put
 *      the backend into any readiness posture and drive install / download /
 *      set-default / delete / transcribe deterministically (no whisper runtime).
 */

// ── Browser capture mocks ────────────────────────────────────────────────────

export interface VoiceBrowserMockOptions {
  /** false → remove getUserMedia + MediaRecorder so the mic hides. Default true. */
  recording?: boolean
  /** true → getUserMedia rejects with NotAllowedError (permission denied). */
  denyPermission?: boolean
}

export async function installVoiceBrowserMocks(
  page: Page,
  options: VoiceBrowserMockOptions = {},
): Promise<void> {
  const recording = options.recording ?? true
  const deny = options.denyPermission ?? false

  await page.addInitScript(
    ({ recording, deny }) => {
      // Force a secure context (getUserMedia + the store's isRecordingSupported
      // both gate on it). On http://localhost it's usually already true, but
      // pin it so the suite is host-agnostic.
      try {
        Object.defineProperty(window, 'isSecureContext', {
          configurable: true,
          get: () => true,
        })
      } catch {
        /* some engines refuse to redefine — best effort */
      }

      if (!recording) {
        // "Unsupported browser" posture: strip the capture APIs so
        // isRecordingSupported() is false and the mic hides.
        try {
          const md = navigator.mediaDevices as unknown as Record<string, unknown>
          if (md) md.getUserMedia = undefined
        } catch {
          /* ignore */
        }
        try {
          Object.defineProperty(window, 'MediaRecorder', {
            configurable: true,
            value: undefined,
          })
        } catch {
          /* ignore */
        }
        return
      }

      // A valid, decodable 16 kHz mono 16-bit PCM WAV (0.1 s tone). The store
      // decodes the recorder blob via AudioContext.decodeAudioData; a garbage
      // blob would reject and route us into the error path, so build a real one.
      function makeWavBlob(): Blob {
        const sampleRate = 16000
        const numSamples = 1600
        const buffer = new ArrayBuffer(44 + numSamples * 2)
        const view = new DataView(buffer)
        const w = (o: number, s: string) => {
          for (let i = 0; i < s.length; i++) view.setUint8(o + i, s.charCodeAt(i))
        }
        w(0, 'RIFF')
        view.setUint32(4, 36 + numSamples * 2, true)
        w(8, 'WAVE')
        w(12, 'fmt ')
        view.setUint32(16, 16, true)
        view.setUint16(20, 1, true)
        view.setUint16(22, 1, true)
        view.setUint32(24, sampleRate, true)
        view.setUint32(28, sampleRate * 2, true)
        view.setUint16(32, 2, true)
        view.setUint16(34, 16, true)
        w(36, 'data')
        view.setUint32(40, numSamples * 2, true)
        for (let i = 0; i < numSamples; i++) {
          view.setInt16(44 + i * 2, Math.round(Math.sin(i / 8) * 3000), true)
        }
        return new Blob([buffer], { type: 'audio/wav' })
      }

      class FakeMediaRecorder {
        stream: unknown
        state = 'inactive'
        mimeType = 'audio/wav'
        ondataavailable: ((e: { data: Blob }) => void) | null = null
        onstop: (() => void) | null = null
        onerror: ((e: unknown) => void) | null = null
        _sliceTimer: ReturnType<typeof setInterval> | null = null
        constructor(stream: unknown) {
          this.stream = stream
        }
        start(timeslice?: number) {
          this.state = 'recording'
          // Real MediaRecorder started with a timeslice flushes a chunk every
          // `timeslice` ms; the live-caption loop relies on that to have audio to
          // decode while recording. Emit a real WAV blob on the same cadence.
          if (typeof timeslice === 'number' && timeslice > 0) {
            this._sliceTimer = setInterval(() => {
              if (this.state === 'recording' && this.ondataavailable) {
                this.ondataavailable({ data: makeWavBlob() })
              }
            }, timeslice)
          }
        }
        requestData() {
          if (this.ondataavailable) this.ondataavailable({ data: makeWavBlob() })
        }
        stop() {
          if (this._sliceTimer) {
            clearInterval(this._sliceTimer)
            this._sliceTimer = null
          }
          if (this.state === 'inactive') {
            if (this.onstop) this.onstop()
            return
          }
          this.state = 'inactive'
          if (this.ondataavailable) this.ondataavailable({ data: makeWavBlob() })
          if (this.onstop) this.onstop()
        }
        pause() {}
        resume() {}
        addEventListener() {}
        removeEventListener() {}
        static isTypeSupported() {
          return true
        }
      }

      try {
        Object.defineProperty(window, 'MediaRecorder', {
          configurable: true,
          writable: true,
          value: FakeMediaRecorder,
        })
      } catch {
        ;(window as unknown as Record<string, unknown>).MediaRecorder =
          FakeMediaRecorder
      }

      const getUserMedia = async () => {
        if (deny) {
          throw new DOMException('Permission denied', 'NotAllowedError')
        }
        return {
          getTracks: () => [
            {
              stop() {},
              kind: 'audio',
              enabled: true,
              readyState: 'live',
            },
          ],
          getAudioTracks: () => [{ stop() {}, kind: 'audio' }],
        }
      }

      try {
        const existing = navigator.mediaDevices
        if (existing) {
          ;(existing as unknown as Record<string, unknown>).getUserMedia =
            getUserMedia
        } else {
          Object.defineProperty(navigator, 'mediaDevices', {
            configurable: true,
            get: () => ({ getUserMedia }),
          })
        }
      } catch {
        try {
          Object.defineProperty(navigator, 'mediaDevices', {
            configurable: true,
            get: () => ({ getUserMedia }),
          })
        } catch {
          /* ignore */
        }
      }
    },
    { recording, deny },
  )
}

// ── /api/voice/** interception ───────────────────────────────────────────────

export interface VoiceApiState {
  capability: VoiceCapability
  settings: VoiceSettings
  versions: RuntimeVersionResponse2[]
  updateCheck: AvailableUpdatesResponse2
  downloads: DownloadSnapshot2[]
  modelStatus: VoiceModelStatus
  instance: VoiceInstanceInfo
  transcribe: TranscriptionResponse
  /** How many times POST /transcribe was called. */
  transcribeCount: number
  /** The interim payload POST /transcribe/stream returns. */
  streamTranscribe: TranscriptionResponse
  /** How many times POST /transcribe/stream was called. */
  streamCount: number
}

export interface VoiceRouteController {
  state: VoiceApiState
  /** Replace the transcribe payload the next POST returns. */
  setTranscribe: (r: TranscriptionResponse) => void
  /** Replace the interim (stream) payload the next POST returns. */
  setStream: (r: TranscriptionResponse) => void
}

const now = () => new Date().toISOString()

function mkVersion(
  version: string,
  isDefault: boolean,
): RuntimeVersionResponse2 {
  return {
    id: `ver-${version}`,
    version,
    backend: 'cpu',
    platform: 'linux',
    arch: 'x86_64',
    binary_path: `/cache/whisper/${version}/whisper-server`,
    is_system_default: isDefault,
    created_at: now(),
  }
}

export function readyCapability(
  over: Partial<VoiceCapability> = {},
): VoiceCapability {
  return {
    enabled: true,
    runtime_ready: true,
    model_ready: true,
    can_transcribe: true,
    model: 'base',
    max_clip_seconds: 60,
    streaming_enabled: true,
    stream_interval_ms: 1000,
    ...over,
  }
}

export function defaultVoiceState(
  over: Partial<VoiceApiState> = {},
): VoiceApiState {
  return {
    capability: readyCapability(),
    settings: {
      enabled: true,
      model: 'base',
      language: 'auto',
      streaming_enabled: true,
      stream_interval_ms: 1000,
      idle_unload_secs: 300,
      auto_start_timeout_secs: 30,
      drain_timeout_secs: 30,
      max_clip_seconds: 60,
      max_upload_bytes: 25 * 1024 * 1024,
      updated_at: now(),
    },
    versions: [mkVersion('v1.0.0', true)],
    updateCheck: {
      platform: 'linux',
      arch: 'x86_64',
      versions: [
        {
          version: 'v1.1.0',
          available_backends: ['cpu'],
          installed_backends: [],
          binary_ready: true,
          installed: false,
          prerelease: false,
          recommended_backend: 'cpu',
          size_bytes: 42 * 1024 * 1024,
          published_at: now(),
        },
        {
          version: 'v1.0.0',
          available_backends: ['cpu'],
          installed_backends: ['cpu'],
          binary_ready: true,
          installed: true,
          prerelease: false,
          recommended_backend: 'cpu',
          size_bytes: 40 * 1024 * 1024,
          published_at: now(),
        },
      ],
    },
    downloads: [],
    modelStatus: { model: 'base', present: true, size_bytes: 147 * 1024 * 1024 },
    instance: {
      status: 'running',
      state: 'healthy',
      active_model: 'ggml-base.bin',
      local_port: 51789,
      restart_attempts: 0,
      state_changed_at: now(),
    },
    transcribe: {
      text: 'hello from the voice engine',
      language: 'en',
      duration_ms: 640,
    },
    transcribeCount: 0,
    streamTranscribe: {
      text: 'hello from the live',
      language: 'en',
      duration_ms: 120,
    },
    streamCount: 0,
    ...over,
  }
}

async function fulfillJson(route: Route, body: unknown, status = 200) {
  await route.fulfill({
    status,
    contentType: 'application/json',
    body: JSON.stringify(body ?? {}),
  })
}

/**
 * Intercept every `/api/voice/**` call and serve it from `state`. Mutating
 * calls (PUT settings, install, set-default, delete, model download,
 * restart/stop, transcribe) mutate `state` so a follow-up reload observes the
 * change — the same way the real backend would.
 */
export async function routeVoice(
  page: Page,
  init: VoiceApiState = defaultVoiceState(),
): Promise<VoiceRouteController> {
  const state = init
  const mkVer = mkVersion

  await page.route('**/api/voice/**', async (route, request) => {
    const method = request.method()
    const url = new URL(request.url())
    const path = url.pathname // e.g. /api/voice/capability
    const seg = path.replace(/^.*\/api\/voice\/?/, '') // e.g. versions/downloads

    // ── GETs ──────────────────────────────────────────────────────────────
    if (method === 'GET') {
      if (seg === 'capability') return fulfillJson(route, state.capability)
      if (seg === 'settings') return fulfillJson(route, state.settings)
      if (seg === 'versions') return fulfillJson(route, { versions: state.versions })
      if (seg === 'versions/check-updates')
        return fulfillJson(route, state.updateCheck)
      if (seg === 'versions/downloads')
        return fulfillJson(route, { downloads: state.downloads })
      if (seg === 'model/status') return fulfillJson(route, state.modelStatus)
      if (seg === 'instance') return fulfillJson(route, state.instance)
      // SSE download-events stream: connected → progress(50%) → complete.
      const evMatch = seg.match(/^versions\/downloads\/([^/]+)\/events$/)
      if (evMatch) {
        const key = decodeURIComponent(evMatch[1])
        const version = key.split('@')[1] ?? 'v1.1.0'
        // Ensure the just-installed version is reflected as installed so the
        // complete-handler's loadVersions()/checkForUpdates() reload shows it.
        if (!state.versions.some(v => v.version === version)) {
          state.versions = [...state.versions, mkVer(version, false)]
        }
        state.updateCheck = {
          ...state.updateCheck,
          versions: state.updateCheck.versions.map(v =>
            v.version === version
              ? { ...v, installed: true, installed_backends: ['cpu'] }
              : v,
          ),
        }
        const body =
          `event: connected\ndata: ${JSON.stringify({ key })}\n\n` +
          `event: progress\ndata: ${JSON.stringify({
            status: 'downloading',
            bytes_received: 524288,
            total_bytes: 1048576,
            percent: 50,
          })}\n\n` +
          `event: complete\ndata: ${JSON.stringify({
            version_id: `ver-${version}`,
            bytes_downloaded: 1048576,
          })}\n\n`
        return route.fulfill({
          status: 200,
          contentType: 'text/event-stream',
          body,
        })
      }
      return fulfillJson(route, {})
    }

    // ── mutations ───────────────────────────────────────────────────────────
    if (seg === 'settings' && method === 'PUT') {
      let body: Record<string, unknown> = {}
      try {
        body = JSON.parse(request.postData() || '{}')
      } catch {
        /* ignore */
      }
      state.settings = { ...state.settings, ...body, updated_at: now() }
      return fulfillJson(route, state.settings)
    }

    if (seg === 'transcribe/stream' && method === 'POST') {
      state.streamCount++
      return fulfillJson(route, state.streamTranscribe)
    }

    if (seg === 'transcribe' && method === 'POST') {
      state.transcribeCount++
      return fulfillJson(route, state.transcribe)
    }

    if (seg === 'model/download' && method === 'POST') {
      state.modelStatus = {
        ...state.modelStatus,
        present: true,
        size_bytes: state.modelStatus.size_bytes ?? 147 * 1024 * 1024,
      }
      return fulfillJson(route, state.modelStatus)
    }

    if (seg === 'versions/download' && method === 'POST') {
      let body: Record<string, unknown> = {}
      try {
        body = JSON.parse(request.postData() || '{}')
      } catch {
        /* ignore */
      }
      const version = (body.version as string) || 'v1.1.0'
      const backend = (body.backend as string) || 'cpu'
      const key = `whisper@${version}@${backend}`
      const snap: DownloadSnapshot2 = {
        task_id: `task-${version}`,
        key,
        version,
        backend,
        status: 'downloading',
        bytes_received: 0,
        total_bytes: 1048576,
      }
      state.downloads = [...state.downloads.filter(d => d.key !== key), snap]
      return fulfillJson(route, {
        task_id: snap.task_id,
        key,
        version,
        backend,
        status: 'downloading',
        events_url: `/api/voice/versions/downloads/${encodeURIComponent(key)}/events`,
      })
    }

    const setDefault = seg.match(/^versions\/([^/]+)\/set-default$/)
    if (setDefault && method === 'POST') {
      const id = decodeURIComponent(setDefault[1])
      state.versions = state.versions.map(v => ({
        ...v,
        is_system_default: v.id === id,
      }))
      const target = state.versions.find(v => v.id === id)
      return fulfillJson(route, target ?? {})
    }

    const del = seg.match(/^versions\/([^/]+)$/)
    if (del && method === 'DELETE') {
      const id = decodeURIComponent(del[1])
      state.versions = state.versions.filter(v => v.id !== id)
      return route.fulfill({ status: 204, body: '' })
    }

    if (seg === 'versions/sync-cache' && method === 'POST') {
      return fulfillJson(route, { added: 0, removed: 0 })
    }

    if (seg === 'instance/restart' && method === 'POST') {
      state.instance = {
        ...state.instance,
        status: 'running',
        state: 'healthy',
        restart_attempts: state.instance.restart_attempts + 1,
        state_changed_at: now(),
      }
      return fulfillJson(route, state.instance)
    }

    if (seg === 'instance/stop' && method === 'POST') {
      state.instance = {
        ...state.instance,
        status: 'stopped',
        state: 'stopped',
        local_port: undefined,
        state_changed_at: now(),
      }
      return fulfillJson(route, state.instance)
    }

    return fulfillJson(route, {})
  })

  return {
    state,
    setTranscribe: (r: TranscriptionResponse) => {
      state.transcribe = r
    },
    setStream: (r: TranscriptionResponse) => {
      state.streamTranscribe = r
    },
  }
}

/** Navigate to the new-chat composer and wait for it to be interactive. */
export async function gotoComposer(page: Page, baseURL: string): Promise<void> {
  await page.goto(`${baseURL}/`)
  await page.waitForLoadState('load')
  await page.waitForSelector('[data-testid="chat-message-textarea"]', {
    timeout: 30000,
  })
}
