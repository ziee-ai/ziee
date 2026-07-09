import { test } from 'node:test'
import assert from 'node:assert/strict'
import type { MessageContent, MessageWithContent } from '@/api-client/types'
import { hasVisibleAnswer, isVisibleAnswerBlock } from './emptyCompletion.ts'

// Minimal content-block factory (only the fields the predicate reads).
function block(content_type: string, content: unknown = {}): MessageContent {
  return {
    id: `blk-${content_type}`,
    message_id: 'm',
    content_type,
    content: content as MessageContent['content'],
    sequence_order: 0,
    created_at: '',
    updated_at: '',
  } as MessageContent
}

function assistant(contents: MessageContent[]): MessageWithContent {
  return { message: {}, contents } as unknown as MessageWithContent
}

test('isVisibleAnswerBlock: reasoning-only / empty text are NOT visible', () => {
  assert.equal(isVisibleAnswerBlock(block('thinking', { thinking: 'x' })), false)
  assert.equal(isVisibleAnswerBlock(block('text', { text: '' })), false)
  assert.equal(isVisibleAnswerBlock(block('text', { text: '   \n\t ' })), false)
})

test('isVisibleAnswerBlock: real answers ARE visible', () => {
  assert.equal(isVisibleAnswerBlock(block('text', { text: 'hi' })), true)
  assert.equal(isVisibleAnswerBlock(block('tool_use', {})), true)
  assert.equal(isVisibleAnswerBlock(block('tool_result', {})), true)
  assert.equal(isVisibleAnswerBlock(block('image', {})), true)
  assert.equal(isVisibleAnswerBlock(block('file_attachment', {})), true)
})

test('hasVisibleAnswer: false for a thinking-only or empty message', () => {
  assert.equal(hasVisibleAnswer(assistant([block('thinking', { thinking: 'reasoning' })])), false)
  assert.equal(hasVisibleAnswer(assistant([])), false)
})

test('hasVisibleAnswer: true when any visible block is present', () => {
  assert.equal(
    hasVisibleAnswer(assistant([block('thinking', { thinking: 'r' }), block('text', { text: 'answer' })])),
    true,
  )
  assert.equal(hasVisibleAnswer(assistant([block('tool_use', {})])), true)
})
