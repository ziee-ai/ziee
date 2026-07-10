import { test } from 'node:test'
import assert from 'node:assert/strict'
import { lineDiff } from './lineDiff.ts'

test('identical inputs yield only context lines', () => {
  const d = lineDiff('a\nb\nc\n', 'a\nb\nc\n')
  assert.ok(d.every(l => l.type === 'ctx'))
  assert.deepEqual(
    d.map(l => l.text),
    ['a', 'b', 'c'],
  )
})

test('detects an added line', () => {
  const d = lineDiff('a\nb\n', 'a\nb\nc\n')
  assert.ok(d.some(l => l.type === 'add' && l.text === 'c'))
  assert.ok(!d.some(l => l.type === 'del'))
})

test('detects a removed line', () => {
  const d = lineDiff('a\nb\nc\n', 'a\nc\n')
  assert.ok(d.some(l => l.type === 'del' && l.text === 'b'))
})

test('detects a changed line as del + add', () => {
  const d = lineDiff('hello\n', 'world\n')
  assert.ok(d.some(l => l.type === 'del' && l.text === 'hello'))
  assert.ok(d.some(l => l.type === 'add' && l.text === 'world'))
})

test('empty inputs are safe', () => {
  assert.deepEqual(lineDiff('', ''), [])
})
