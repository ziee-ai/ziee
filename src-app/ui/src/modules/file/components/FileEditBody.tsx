import { useEffect, useRef, useState } from 'react'
import { Button, Spin, message } from '@/components/ui'
import { ApiClient } from '@/api-client'
import type { File as FileEntity } from '@/api-client/types'
import { Stores } from '@/core/stores'
import { LazyMarkdownEditor } from '@/components/kit/editor/LazyMarkdownEditor'
import { LazyCodeEditor } from '@/components/kit/editor/LazyCodeEditor'
import type { CanvasEditorHandle } from '@/components/kit/editor/types'
import { editableKind } from '@/modules/file/utils/editableTypes'

/**
 * The canvas edit-mode body: loads the file's head markdown, mounts the Plate
 * WYSIWYG editor, and Saves the result as a new version (the user side of
 * co-editing a deliverable). Explicit Save — the editor is read on Save only.
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
  const editorRef = useRef<CanvasEditorHandle>(null)
  const kind = editableKind(file)

  useEffect(() => {
    let cancelled = false
    void (async () => {
      try {
        const res = await ApiClient.File.getTextContent({ file_id: file.id })
        const t = typeof res === 'string' ? res : await (res as Blob).text()
        if (!cancelled) setText(t)
      } catch {
        if (!cancelled) setText('')
      }
    })()
    return () => {
      cancelled = true
    }
  }, [file.id])

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
      <div className="flex-1 overflow-hidden">
        {kind === 'code' ? (
          <LazyCodeEditor
            ref={editorRef}
            initialText={text}
            onDirty={() => setDirty(true)}
          />
        ) : (
          <LazyMarkdownEditor
            ref={editorRef}
            initialMarkdown={text}
            onDirty={() => setDirty(true)}
          />
        )}
      </div>
      <div className="flex items-center justify-end gap-2 border-border border-t px-3 py-2">
        <Button
          variant="ghost"
          onClick={onDone}
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
