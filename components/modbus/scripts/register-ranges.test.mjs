import { test } from 'node:test'
import assert from 'node:assert/strict'
import { rangesValid, rangeError, rangeCounts } from '../frontend/src/utils/registerRanges.ts'
const range = (RegisterType, StartAddress, EndAddress, DataType = 'uint16') => ({ RegisterType, StartAddress, EndAddress, DataType, Endian: 'big' })
test('empty and mixed disjoint areas preserve typed point counts', () => {
  assert(rangesValid([]))
  assert(rangesValid([range('holding_register', 0, 9), range('holding_register', 100, 119, 'float32'), range('input_register', 0, 1, 'uint32')]))
  assert.deepEqual(rangeCounts(range('holding_register', 100, 119, 'float32')), { points: 10, addresses: 20 })
  assert(rangesValid([range('coil', 65535, 65535, 'bool')]))
})
test('incomplete, overlapping, invalid and missing addresses are rejected', () => {
  assert.equal(rangeError([range('holding_register', 0, 2, 'float32')], 0), 'ranges.incompletePoint')
  assert.equal(rangeError([range('coil', 0, 2)], 0), 'ranges.invalidType')
  assert.equal(rangeError([range('holding_register', 0, 1), range('holding_register', 1, 3)], 0), 'ranges.overlap')
  for (const [start, end] of [[NaN, 1], ['', 1], [-1, 1], [2, 1], [0.5, 1], [65535, 65536]]) {
    assert.equal(rangeError([range('holding_register', start, end)], 0), 'ranges.invalidAddress')
  }
})
