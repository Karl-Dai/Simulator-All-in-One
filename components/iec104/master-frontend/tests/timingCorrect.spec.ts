import { describe, it, expect } from 'vitest'
import { correctTimingEdit, formatCorrections, isTimingField, type TimingFields } from '@shared/timing'

// Mirrors the spec scenarios for the edit-aware C3 frontend correction.
// The same shared util backs both the master and slave forms, so this also
// covers "master 与 slave 行为一致".

const base = (): TimingFields => ({ t0: 30, t1: 15, t2: 10, t3: 20, k: 12, w: 8 })

describe('correctTimingEdit (C3, t1/k anchors)', () => {
  it('valid config is a no-op', () => {
    const f = base()
    expect(correctTimingEdit(f, 't1')).toEqual([])
    expect(f).toEqual(base())
  })

  it('editing t1 down pushes only t2 (no cascade)', () => {
    const f = base()
    f.t1 = 5
    const changes = correctTimingEdit(f, 't1')
    expect(f.t1).toBe(5)
    expect(f.t2).toBe(4)
    expect(f.t3).toBe(20)
    expect(changes).toEqual([{ field: 't2', from: 10, to: 4 }])
  })

  it('editing t1 above t3 pushes only t3', () => {
    const f = base()
    f.t1 = 50
    correctTimingEdit(f, 't1')
    expect(f.t1).toBe(50)
    expect(f.t3).toBe(51)
    expect(f.t2).toBe(10)
  })

  it('editing t2 above t1 clamps t2, leaves anchor', () => {
    const f = base()
    f.t2 = 20
    const changes = correctTimingEdit(f, 't2')
    expect(f.t2).toBe(14)
    expect(f.t1).toBe(15)
    expect(f.t3).toBe(20)
    expect(changes).toEqual([{ field: 't2', from: 20, to: 14 }])
  })

  it('editing t3 below t1 clamps t3 up', () => {
    const f = base()
    f.t3 = 8
    correctTimingEdit(f, 't3')
    expect(f.t3).toBe(16)
    expect(f.t1).toBe(15)
  })

  it('editing k clamps w to floor(2k/3)', () => {
    const f = base()
    f.k = 10 // floor(20/3) = 6
    correctTimingEdit(f, 'k')
    expect(f.w).toBe(6)
    expect(f.k).toBe(10)
  })

  it('editing w above bound clamps w', () => {
    const f = base()
    f.w = 99
    correctTimingEdit(f, 'w')
    expect(f.w).toBe(8) // floor(2*12/3)
  })

  it('t1 anchor is clamped at the low rail', () => {
    const f = base()
    f.t1 = 1
    correctTimingEdit(f, 't1')
    expect(f.t1).toBe(2)
    expect(f.t2).toBe(1)
  })

  it('t1 anchor is clamped at the high rail', () => {
    const f = base()
    f.t1 = 255
    correctTimingEdit(f, 't1')
    expect(f.t1).toBe(254)
    expect(f.t3).toBe(255)
  })

  it('t0 is independent of the triple', () => {
    const f = base()
    f.t0 = 0
    const changes = correctTimingEdit(f, 't0')
    expect(f.t0).toBe(1)
    expect([f.t1, f.t2, f.t3]).toEqual([15, 10, 20])
    expect(changes).toEqual([{ field: 't0', from: 0, to: 1 }])
  })

  it('coerces non-finite (empty) input to the lower bound', () => {
    const f = base()
    f.t2 = NaN
    correctTimingEdit(f, 't2')
    expect(f.t2).toBe(1)
  })
})

describe('helpers', () => {
  it('isTimingField recognizes the six timing keys only', () => {
    expect(isTimingField('t1')).toBe(true)
    expect(isTimingField('w')).toBe(true)
    expect(isTimingField('default_qoi')).toBe(false)
  })

  it('formatCorrections renders symbolic detail', () => {
    expect(formatCorrections([{ field: 't2', from: 10, to: 4 }])).toBe('t2: 10 → 4')
  })
})
