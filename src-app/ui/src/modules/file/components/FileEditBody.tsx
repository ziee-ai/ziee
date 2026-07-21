import { useEffect, useRef, useState } from 'react'
import { TriangleAlert } from 'lucide-react'
import { Button, Spin, message } from '@ziee/kit'
import { ApiClient } from '@/api-client'
import type { File as FileEntity } from '@/api-client/types'
import { LazyMarkdownEditor } from '@/components/kit/editor/LazyMarkdownEditor'
import { LazyCodeEditor } from '@/components/kit/editor/LazyCodeEditor'
import { CsvGridEditor } from '@/modules/file/components/CsvGridEditor'
import { CanvasSelectionPopover } from '@/modules/file/components/CanvasSelectionPopover'
import type { CanvasEditorHandle } from '@/components/kit/editor/types'
import { editableKind } from '@/modules/file/utils/editableTypes'
import { FileVersions as FileVersionsStore } from '@/modules/file/stores/fileVersions'

/**
 * The canvas edit-mode body: loads the file's head content, mounts the
 * type-appropriate editor (markdown → Plate, code → CodeMirror), and Saves the
 * result as a new version (the user side of co-editing a deliverable). Explicit
 * Save — the editor is read on Save only.
 *
 * Concurrent-edit safety: if the head advances underneath the editor (a model
 * `edit_file`/`rewrite_file` or another device) it shows a non-destructive
 * banner — Reload latest (discard local) or Keep my changes (Save appends a new
 * head; nothing is lost). Never a silent overwrite.
 */
export function FileEditBody({
  file,
  onDone,
}: {
  file: FileEntity
  onDone: () => void
}) {
  const [text, setText] = useState<string | null>(null)
  const [loadError, setLoadError] = useState(false)
  const [saving, setSaving] = useState(false)
  const [dirty, setDirty] = useState(false)
  const [dismissedChange, setDismissedChange] = useState(false)
  const [reloadKey, setReloadKey] = useState(0)
  const editorRef = useRef<CanvasEditorHandle>(null)
  const bodyRef = useRef<HTMLDivElement>(null)
  // Head version the currently-loaded text was fetched from.
  const loadedHeadRef = useRef<number | null>(null)
  const kind = editableKind(file)

  // Reactive current head (authoritative: FileVersions list, else the prop).
  const versionsByFile = FileVersionsStore.versionsByFile
  const currentHead =
    versionsByFile.get(file.id)?.find(v => v.is_head)?.version ?? file.version
  const changedUnderneath =
    loadedHeadRef.current !== null &&
    currentHead > loadedHeadRef.current &&
    !dismissedChange &&
    // Suppress during our OWN save: appendVersion bumps the head (via loadVersions)
    // before onDone() unmounts us, which would otherwise flash a false "changed
    // elsewhere" banner for the version we just wrote.
    !saving

  useEffect(() => {
    let cancelled = false
    void (async () => {
      try {
        const res = await ApiClient.File.getTextContent({ file_id: file.id })
        const t = typeof res === 'string' ? res : await (res as Blob).text()
        if (cancelled) return
        setLoadError(false)
        setText(t)
        // Snapshot the head this text came from (read state in handlers via `$`).
        const head =
          FileVersionsStore.$.versionsByFile
            .get(file.id)
            ?.find(v => v.is_head)?.version ?? file.version
        loadedHeadRef.current = head
        setDismissedChange(false)
        setDirty(false)
      } catch {
        // Data-loss guard: do NOT fall back to an empty editor. An empty editor
        // over a failed load would let Save append a blank version that clobbers
        // the real head content. Surface an error + Retry; Save stays unreachable.
        if (!cancelled) setLoadError(true)
      }
    })()
    return () => {
      cancelled = true
    }
    // reloadKey drives the re-fetch on "Reload latest".
  }, [file.id, reloadKey])

  // Guard against navigate-away / reload / close with unsaved edits.
  useEffect(() => {
    if (!dirty) return
    const handler = (e: BeforeUnloadEvent) => {
      e.preventDefault()
      e.returnValue = ''
    }
    window.addEventListener('beforeunload', handler)
    return () => window.removeEventListener('beforeunload', handler)
  }, [dirty])

  const handleSave = async () => {
    const content = editorRef.current?.getContent() ?? ''
    setSaving(true)
    try {
      await FileVersionsStore.appendVersion(file.id, content)
      message.success('Saved')
      onDone()
    } catch (e) {
      console.error('[canvas] save failed', e)
      message.error('Failed to save')
    } finally {
      setSaving(false)
    }
  }

  if (loadError) {
    return (
      <div
        className="flex h-full flex-col items-center justify-center gap-3 p-6 text-center"
        data-testid="canvas-load-error"
      >
        <TriangleAlert className="size-8 text-warning" />
        <div className="text-muted-foreground text-sm">
          Couldn’t load this document to edit. Its content was not changed.
        </div>
        <div className="flex gap-2">
          <Button
            variant="outline"
            onClick={() => {
              setLoadError(false)
              setText(null)
              setReloadKey(k => k + 1)
            }}
            data-testid="canvas-load-retry"
          >
            Retry
          </Button>
          <Button variant="ghost" onClick={onDone} data-testid="canvas-load-cancel">
            Cancel
          </Button>
        </div>
      </div>
    )
  }

  if (text === null) {
    return (
      <div className="flex h-full items-center justify-center">
        <Spin label="Loading" />
      </div>
    )
  }

  return (
    <div ref={bodyRef} className="flex h-full flex-col" data-testid="canvas-edit-body">
      {/* Selection → LLM popover (markdown canvas only): "Ask about this" quotes
          the excerpt into the composer (ITEM-15); "Edit this section" sends a
          scoped edit (ITEM-16). */}
      {kind === null || kind === 'markdown' ? (
        <CanvasSelectionPopover
          containerRef={bodyRef}
          fileName={file.filename}
          getDocText={() => editorRef.current?.getContent() ?? ''}
        />
      ) : null}
      {changedUnderneath && (
        <div
          className="flex flex-wrap items-center gap-2 border-warning/40 border-b bg-warning/10 px-3 py-2 text-sm text-warning"
          data-testid="canvas-changed-banner"
        >
          <TriangleAlert className="size-4" />
          <span className="flex-1">This document changed elsewhere.</span>
          <Button
            variant="outline"
            disabled={saving}
            onClick={() => {
              // Unmount the editor (text=null → spinner) so it REMOUNTS with the
              // freshly-fetched content. Without the null, bumping reloadKey
              // remounts synchronously with the still-stale `text` (the async
              // refetch lands after) and Plate's usePlateEditor never rebuilds on
              // a prop-only value change — so the reload would show old content.
              setText(null)
              setReloadKey(k => k + 1)
            }}
            data-testid="canvas-reload-latest"
          >
            Reload latest
          </Button>
          <Button
            variant="ghost"
            onClick={() => setDismissedChange(true)}
            data-testid="canvas-keep-mine"
          >
            Keep my changes
          </Button>
        </div>
      )}
      <div className="flex-1 overflow-hidden">
        {kind === 'csv' ? (
          <CsvGridEditor
            key={reloadKey}
            ref={editorRef}
            initialText={text}
            onDirty={() => setDirty(true)}
          />
        ) : kind === 'code' ? (
          <LazyCodeEditor
            key={reloadKey}
            ref={editorRef}
            initialText={text}
            onDirty={() => setDirty(true)}
          />
        ) : (
          <LazyMarkdownEditor
            key={reloadKey}
            ref={editorRef}
            initialMarkdown={text}
            onDirty={() => setDirty(true)}
          />
        )}
      </div>
      <div className="flex items-center justify-end gap-2 border-border border-t px-3 py-2">
        <Button
          variant="ghost"
          onClick={() => {
            // Confirm before discarding unsaved edits (parity with the
            // beforeunload guard — Cancel is an equally common exit path).
            if (!dirty || window.confirm('Discard unsaved changes?')) onDone()
          }}
          disabled={saving}
          data-testid="canvas-cancel"
        >
          Cancel
        </Button>
        <Button
          variant="default"
          onClick={handleSave}
          disabled={saving || !dirty}
          data-testid="canvas-save"
        >
          {saving ? 'Saving…' : 'Save'}
        </Button>
      </div>
    </div>
  )
}
