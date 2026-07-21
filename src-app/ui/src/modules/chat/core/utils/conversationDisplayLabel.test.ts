import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  conversationDisplayLabel,
  UNTITLED_CONVERSATION_LABEL,
} from './conversationDisplayLabel.ts'

// TEST-14: display-label precedence (ITEM-7). Pure, no DOM.

test('a real title wins over the preview', () => {
  assert.equal(
    conversationDisplayLabel({
      title: 'BRCA1 in Hereditary Breast Cancer',
      first_message_preview: 'what does BRCA1 do',
    }),
    'BRCA1 in Hereditary Breast Cancer',
  )
})

test('falls back to the first-message preview when the title is null', () => {
  assert.equal(
    conversationDisplayLabel({
      title: null,
      first_message_preview: 'what does BRCA1 do',
    }),
    'what does BRCA1 do',
  )
})

test('falls back to the placeholder when neither is present', () => {
  assert.equal(
    conversationDisplayLabel({ title: null, first_message_preview: null }),
    UNTITLED_CONVERSATION_LABEL,
  )
})

test('a whitespace-only title is treated as absent', () => {
  // Matches the backend `has_title` semantics: a blank title renders as an
  // empty row, which is strictly worse than the placeholder.
  assert.equal(
    conversationDisplayLabel({
      title: '   \n ',
      first_message_preview: 'what does BRCA1 do',
    }),
    'what does BRCA1 do',
  )
})

test('a whitespace-only preview also falls through to the placeholder', () => {
  assert.equal(
    conversationDisplayLabel({ title: null, first_message_preview: '  ' }),
    UNTITLED_CONVERSATION_LABEL,
  )
})

test('the returned label is trimmed', () => {
  assert.equal(
    conversationDisplayLabel({ first_message_preview: '  padded question  ' }),
    'padded question',
  )
})

test('a missing/undefined conversation yields the placeholder', () => {
  assert.equal(conversationDisplayLabel(undefined), UNTITLED_CONVERSATION_LABEL)
  assert.equal(conversationDisplayLabel(null), UNTITLED_CONVERSATION_LABEL)
  assert.equal(conversationDisplayLabel({}), UNTITLED_CONVERSATION_LABEL)
})
