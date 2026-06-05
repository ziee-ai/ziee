import { getAuthToken, getBaseUrl } from '@/api-client/core'
import type { SyncEvent } from '@/api-client/types'
import { Stores } from '@/core/stores'
import { setSyncConnectionId } from './connection'
import { resyncAll } from './registry'

// Realtime-sync SSE client. A thin bridge: opens the per-user
// `GET /api/sync/subscribe` stream and re-emits each `{entity, action,
// id}` frame onto the client EventBus as a per-entity `sync:<entity>`
// event. Existing per-module handlers (registerSync) then refetch. Reuses
// the same fetch + ReadableStream approach as the api-client so header
// auth works (EventSource can't set Authorization).

const INITIAL_BACKOFF_MS = 1_000
const MAX_BACKOFF_MS = 30_000

let started = false
let abort: AbortController | null = null
let backoffMs = INITIAL_BACKOFF_MS

/** Start the sync stream (idempotent). Call when a user is authenticated. */
export function startSyncClient(): void {
  if (started) return
  started = true
  backoffMs = INITIAL_BACKOFF_MS
  void connectLoop()
}

/** Stop the sync stream and clear the connection id. Call on logout. */
export function stopSyncClient(): void {
  started = false
  abort?.abort()
  abort = null
  setSyncConnectionId(null)
}

async function connectLoop(): Promise<void> {
  while (started) {
    try {
      await connectOnce()
    } catch (error) {
      if (!started) break
      if (!(error instanceof DOMException && error.name === 'AbortError')) {
        console.warn('[sync] stream ended; reconnecting', error)
      }
    }
    if (!started) break
    await delay(backoffMs)
    backoffMs = Math.min(backoffMs * 2, MAX_BACKOFF_MS)
  }
}

async function connectOnce(): Promise<void> {
  const token = getAuthToken()
  if (!token) throw new Error('[sync] no auth token')

  const baseUrl = await getBaseUrl()
  abort = new AbortController()

  const response = await fetch(`${baseUrl}/api/sync/subscribe`, {
    headers: {
      Authorization: `Bearer ${token}`,
      Accept: 'text/event-stream',
    },
    signal: abort.signal,
  })

  if (!response.ok || !response.body) {
    throw new Error(`[sync] subscribe failed: ${response.status}`)
  }

  // Connected: reset backoff and resync to cover anything missed while
  // the stream was down (best-effort durability).
  backoffMs = INITIAL_BACKOFF_MS
  resyncAll()

  const reader = response.body.getReader()
  const decoder = new globalThis.TextDecoder()
  let buffer = ''
  let currentEvent = ''

  while (started) {
    const { done, value } = await reader.read()
    if (done) break

    buffer += decoder.decode(value, { stream: true })
    const lines = buffer.split('\n')
    buffer = lines.pop() || ''

    for (const line of lines) {
      if (line.trim() === '') {
        currentEvent = ''
        continue
      }
      if (line.startsWith('event: ')) {
        currentEvent = line.slice(7).trim()
      } else if (line.startsWith('data: ')) {
        const raw = line.slice(6)
        let parsed: unknown = raw
        try {
          parsed = JSON.parse(raw)
        } catch {
          // keep as string
        }
        handleFrame(currentEvent, parsed)
      }
    }
  }
}

function handleFrame(event: string, data: unknown): void {
  if (event === 'connected') {
    const connId = (data as { connection_id?: string } | null)?.connection_id
    if (typeof connId === 'string') {
      setSyncConnectionId(connId)
    }
    return
  }

  if (event === 'sync') {
    const ev = data as SyncEvent | null
    if (!ev || !ev.entity || !ev.action || !ev.id) return
    // Re-emit onto the existing EventBus as a per-entity event; the
    // module's registerSync handler reacts. Cast: the template-literal
    // key is a valid `keyof AppEvents` but TS can't narrow it from the
    // runtime entity string.
    void Stores.EventBus.emit({
      type: `sync:${ev.entity}`,
      data: { action: ev.action, id: ev.id },
    } as never)
  }
}

function delay(ms: number): Promise<void> {
  return new Promise(resolve => globalThis.setTimeout(resolve, ms))
}
