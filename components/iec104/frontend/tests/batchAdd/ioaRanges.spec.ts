import { describe, it, expect } from 'vitest'
import { compressRanges, lowerBound, findNextFreeGap, parseIoaExpression, resolveIoaHits, IOA_MAX } from '../../src/components/batchAdd/ioaRanges'

describe('compressRanges', () => {
  it('returns empty string for []', () => {
    expect(compressRanges([])).toBe('')
  })

  it('handles single value', () => {
    expect(compressRanges([5])).toBe('5')
  })

  it('compresses one contiguous run', () => {
    expect(compressRanges([0, 1, 2, 3])).toBe('0–3')
  })

  it('compresses multiple runs with gaps', () => {
    expect(compressRanges([0, 1, 2, 5, 7, 8])).toBe('0–2, 5, 7–8')
  })

  it('keeps singletons as singletons', () => {
    expect(compressRanges([1, 3, 5])).toBe('1, 3, 5')
  })
})

describe('lowerBound', () => {
  it('returns 0 when target ≤ first element', () => {
    expect(lowerBound([10, 20, 30], 5)).toBe(0)
    expect(lowerBound([10, 20, 30], 10)).toBe(0)
  })

  it('returns length when target > last element', () => {
    expect(lowerBound([10, 20, 30], 100)).toBe(3)
  })

  it('returns index of first element ≥ target', () => {
    expect(lowerBound([10, 20, 30], 20)).toBe(1)
    expect(lowerBound([10, 20, 30], 25)).toBe(2)
  })

  it('handles empty input', () => {
    expect(lowerBound([], 5)).toBe(0)
  })
})

describe('findNextFreeGap', () => {
  it('returns 0 when there are no existing points', () => {
    expect(findNextFreeGap([], 10)).toBe(0)
  })

  it('returns the gap before the first range when it fits', () => {
    // existing 50..59, want 20 — fits at 0..19
    expect(findNextFreeGap([50, 51, 52, 53, 54, 55, 56, 57, 58, 59], 20)).toBe(0)
  })

  it('skips past first range when count would overlap it', () => {
    // existing 0..9, want 5 — 0..4 overlaps, so jump to 10
    expect(findNextFreeGap([0, 1, 2, 3, 4, 5, 6, 7, 8, 9], 5)).toBe(10)
  })

  it('finds the inner gap when first slot too small', () => {
    // existing 0..9 and 50..59, want 20 — gap 10..49 fits → 10
    const xs = [...Array(10).keys(), ...Array.from({ length: 10 }, (_, i) => 50 + i)]
    expect(findNextFreeGap(xs, 20)).toBe(10)
  })

  it('jumps past both ranges when no inner gap fits', () => {
    // existing 0..9 and 50..59, want 60 — 10..49 too small, jump to 60
    const xs = [...Array(10).keys(), ...Array.from({ length: 10 }, (_, i) => 50 + i)]
    expect(findNextFreeGap(xs, 60)).toBe(60)
  })

  it('returns null when no fitting gap exists below IOA_MAX', () => {
    // existing [0, 1], want IOA_MAX points — s pushed to 2, end = IOA_MAX+1 > IOA_MAX
    expect(findNextFreeGap([0, 1], IOA_MAX)).toBeNull()
  })

  it('returns 0 with count = 1 and existing [5]', () => {
    expect(findNextFreeGap([5], 1)).toBe(0)
  })
})

describe('parseIoaExpression', () => {
  it('空串 → 空结果无错', () => {
    expect(parseIoaExpression('')).toEqual({ ranges: [], singles: [], error: null })
    expect(parseIoaExpression('   ')).toEqual({ ranges: [], singles: [], error: null })
  })

  it('单点 + 多分隔符（逗号/空格/换行）', () => {
    expect(parseIoaExpression('100, 200 300\n400')).toEqual({
      ranges: [], singles: [100, 200, 300, 400], error: null,
    })
  })

  it('区间', () => {
    expect(parseIoaExpression('1000-2000')).toEqual({
      ranges: [[1000, 2000]], singles: [], error: null,
    })
  })

  it('单点与区间混合 + 单点去重排序', () => {
    expect(parseIoaExpression('5000, 100, 100, 1000-2000')).toEqual({
      ranges: [[1000, 2000]], singles: [100, 5000], error: null,
    })
  })

  it('等值区间 a-a 合法', () => {
    expect(parseIoaExpression('5-5')).toEqual({ ranges: [[5, 5]], singles: [], error: null })
  })

  it('非数字 token → error 置该 token', () => {
    expect(parseIoaExpression('100, abc').error).toBe('abc')
  })

  it('区间反向 b<a → error', () => {
    expect(parseIoaExpression('200-100').error).toBe('200-100')
  })

  it('单点越域 > IOA_MAX → error', () => {
    expect(parseIoaExpression(String(IOA_MAX + 1)).error).toBe(String(IOA_MAX + 1))
  })

  it('区间上界越域 → error', () => {
    expect(parseIoaExpression('0-99999999').error).toBe('0-99999999')
  })

  it('带空格的破折号视为非法 token', () => {
    expect(parseIoaExpression('100 - 200').error).toBe('-')
  })
})

describe('resolveIoaHits', () => {
  const existing = [100, 1000, 1500, 2000] // 升序去重

  it('区间过滤已存在点（稀疏区间天然成立）', () => {
    const r = resolveIoaHits(parseIoaExpression('1000-2000'), existing)
    expect(r).toEqual({ hitIoas: [1000, 1500, 2000], missedSingles: [] })
  })

  it('单点命中 + 缺失单点计入 missed', () => {
    const r = resolveIoaHits(parseIoaExpression('100, 999'), existing)
    expect(r).toEqual({ hitIoas: [100], missedSingles: [999] })
  })

  it('区间与单点并集去重', () => {
    const r = resolveIoaHits(parseIoaExpression('1000-1500, 1000'), existing)
    expect(r).toEqual({ hitIoas: [1000, 1500], missedSingles: [] })
  })

  it('区间命中不计 missed（即便区间内多数 IOA 不存在）', () => {
    const r = resolveIoaHits(parseIoaExpression('0-100000'), existing)
    expect(r).toEqual({ hitIoas: [100, 1000, 1500, 2000], missedSingles: [] })
  })

  it('语法错 → 空命中', () => {
    const r = resolveIoaHits(parseIoaExpression('abc'), existing)
    expect(r).toEqual({ hitIoas: [], missedSingles: [] })
  })

  it('空 existing → 单点全 missed、区间空', () => {
    const r = resolveIoaHits(parseIoaExpression('100, 1-9'), [])
    expect(r).toEqual({ hitIoas: [], missedSingles: [100] })
  })
})
