import { test } from 'node:test'
import assert from 'node:assert/strict'
import type { OpenDoc } from '@/api-client/types'
import {
  OFFICE_DOCS_SYNC_EVENTS,
  refetchOpenDocuments,
} from './officeBridgeSync.ts'

// TEST-17 [covers ITEM-14] — the OfficeBridge store's notify-and-refetch core:
// a `sync:office_document` notify maps to a refetch that exposes the open-docs
// list, self-gated on `office_bridge::use` (the no-403 rule). Exercised through
// the dependency-free `refetchOpenDocuments` the store wires its listeners to.
// (`import type` is erased at runtime, so this runs under `node --test`.)

const sampleDocs: OpenDoc[] = [
  {
    app: 'word',
    name: 'Report.docx',
    full_name: 'C:/Users/test/Report.docx',
    path: 'C:/Users/test',
    saved: true,
    active: true,
    attach_method: 'mock',
  },
  {
    app: 'excel',
    name: 'Budget.xlsx',
    full_name: 'C:/Users/test/Budget.xlsx',
    path: 'C:/Users/test',
    saved: false,
    active: false,
    attach_method: 'mock',
  },
]

test('the store subscribes to office_document + reconnect sync events', () => {
  assert.ok(
    OFFICE_DOCS_SYNC_EVENTS.includes('sync:office_document'),
    'must refetch on the office_document open/close notify',
  )
  assert.ok(
    OFFICE_DOCS_SYNC_EVENTS.includes('sync:reconnect'),
    'must resync on reconnect',
  )
})

test('a sync notify refetches and exposes the open-documents list', async () => {
  let fetched = 0
  let exposed: OpenDoc[] | null = null
  let pushedToPanel: OpenDoc[] | null = null
  const loadingStates: boolean[] = []

  await refetchOpenDocuments({
    hasUsePermission: () => true,
    fetchDocuments: async () => {
      fetched++
      return sampleDocs
    },
    setDocuments: docs => {
      exposed = docs
    },
    setLoading: v => loadingStates.push(v),
    pushToOpenPanel: docs => {
      pushedToPanel = docs
    },
  })

  assert.equal(fetched, 1, 'the notify triggers exactly one refetch')
  assert.deepEqual(exposed, sampleDocs, 'the refetched list is exposed to the panel')
  assert.deepEqual(pushedToPanel, sampleDocs, 'the fresh list is pushed into the open panel tab')
  assert.deepEqual(loadingStates, [true, false], 'loading flips on then off')
})

test('self-gates on office_bridge::use — no refetch without the permission', async () => {
  let fetched = 0
  const loadingStates: boolean[] = []

  await refetchOpenDocuments({
    hasUsePermission: () => false,
    fetchDocuments: async () => {
      fetched++
      return sampleDocs
    },
    setDocuments: () => {},
    setLoading: v => loadingStates.push(v),
  })

  assert.equal(fetched, 0, 'must not hit the endpoint without office_bridge::use (the endpoint would 403)')
  assert.deepEqual(loadingStates, [], 'no loading toggling when gated out')
})

test('a fetch failure is reported and loading is always cleared', async () => {
  let reportedMessage: unknown = null
  const loadingStates: boolean[] = []

  await refetchOpenDocuments({
    hasUsePermission: () => true,
    fetchDocuments: async () => {
      throw new Error('boom')
    },
    setDocuments: () => {
      throw new Error('setDocuments must not run on failure')
    },
    setLoading: v => loadingStates.push(v),
    onError: err => {
      reportedMessage = (err as { message?: unknown } | null)?.message ?? null
    },
  })

  assert.equal(reportedMessage, 'boom', 'the failure is surfaced to onError')
  assert.deepEqual(loadingStates, [true, false], 'loading is cleared even when the refetch throws')
})
