import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  buildSelectionAskMessage,
  buildSelectionEditMessage,
  isUniqueSelection,
} from './selectionEdit.ts'

const DOC = '# Report\n\nThe assay was run twice. The assay was validated.\n'

test('isUniqueSelection: unique substring', () => {
  assert.equal(isUniqueSelection(DOC, 'validated'), true)
})

test('isUniqueSelection: repeated substring is not unique', () => {
  assert.equal(isUniqueSelection(DOC, 'The assay'), false)
})

test('isUniqueSelection: absent / empty', () => {
  assert.equal(isUniqueSelection(DOC, 'missing'), false)
  assert.equal(isUniqueSelection(DOC, ''), false)
})

test('edit message includes oldStr when selection is unique', () => {
  const r = buildSelectionEditMessage('report.md', 'validated', 'expand this', DOC)
  assert.equal(r.oldStr, 'validated')
  assert.match(r.message, /edit exactly this section/)
  assert.match(r.message, /> validated/)
})

test('edit message omits oldStr when selection is ambiguous', () => {
  const r = buildSelectionEditMessage('report.md', 'The assay', 'expand this', DOC)
  assert.equal(r.oldStr, undefined)
  assert.match(r.message, /appears more than once/)
})

test('ask message quotes the excerpt', () => {
  const m = buildSelectionAskMessage('validated', 'what does this mean?')
  assert.match(m, /> validated/)
  assert.match(m, /what does this mean\?/)
})
