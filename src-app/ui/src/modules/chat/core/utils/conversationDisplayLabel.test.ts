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

// TEST-27 — the preview rung is the user's RAW first message, so it carries the
// LaTeX they typed. No list surface renders markdown, so the label must already
// be the plain-text reading. Both rungs go through it: a legacy `title` that was
// persisted as the raw message needs the same treatment.
test('math delimiters are resolved in both the title and the preview rungs', () => {
  assert.equal(
    conversationDisplayLabel({
      first_message_preview: 'Check: the energy is \\[ E = mc^2 \\] where \\( m \\) ...',
    }),
    'Check: the energy is E = mc^2 where m ...',
  )
  assert.equal(
    conversationDisplayLabel({ title: 'Deriving \\( \\lambda \\) decay' }),
    'Deriving \\lambda decay',
  )
  // ...and a label whose ONLY content was a delimiter pair still resolves rather
  // than collapsing to the placeholder
  assert.equal(conversationDisplayLabel({ title: '\\( x \\)' }), 'x')
})

test('a missing/undefined conversation yields the placeholder', () => {
  assert.equal(conversationDisplayLabel(undefined), UNTITLED_CONVERSATION_LABEL)
  assert.equal(conversationDisplayLabel(null), UNTITLED_CONVERSATION_LABEL)
  assert.equal(conversationDisplayLabel({}), UNTITLED_CONVERSATION_LABEL)
})
