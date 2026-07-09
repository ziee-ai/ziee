import { useEffect, useRef, useState } from 'react'
import { TriangleAlert } from 'lucide-react'
import { Button, Spin, message } from '@/components/ui'
import { ApiClient } from '@/api-client'
import type { File as FileEntity } from '@/api-client/types'
import { Stores } from '@/core/stores'
import { LazyMarkdownEditor } from '@/components/kit/editor/LazyMarkdownEditor'
import { LazyCodeEditor } from '@/components/kit/editor/LazyCodeEditor'
import type { CanvasEditorHandle } from '@/components/kit/editor/types'
import { editableKind } from '@/modules/file/utils/editableTypes'

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
  const [saving, setSaving] = useState(false)
  const [dirty, setDirty] = useState(false)
  const [dismissedChange, setDismissedChange] = useState(false)
  const [reloadKey, setReloadKey] = useState(0)
  const editorRef = useRef<CanvasEditorHandle>(null)
  // Head version the currently-loaded text was fetched from.
  const loadedHeadRef = useRef<number | null>(null)
  const kind = editableKind(file)

  // Reactive current head (authoritative: FileVersions list, else the prop).
  const versionsByFile = Stores.FileVersions.versionsByFile
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
        setText(t)
        // Snapshot the head this text came from (read state in handlers via `$`).
        const head =
          Stores.FileVersions.$.versionsByFile
            .get(file.id)
            ?.find(v => v.is_head)?.version ?? file.version
        loadedHeadRef.current = head
        setDismissedChange(false)
        setDirty(false)
      } catch {
        if (!cancelled) setText('')
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
      await Stores.FileVersions.appendVersion(file.id, content)
      message.success('Saved')
      onDone()
    } catch (e) {
      console.error('[canvas] save failed', e)
      message.error('Failed to save')
    } finally {
      setSaving(false)
    }
  }

  if (text === null) {
    return (
      <div className="flex h-full items-center justify-center">
        <Spin label="Loading" />
      </div>
    )
  }

  return (
    <div className="flex h-full flex-col" data-testid="canvas-edit-body">
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
        {kind === 'code' ? (
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
