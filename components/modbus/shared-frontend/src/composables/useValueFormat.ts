import { computed, type Ref } from 'vue'

/**
 * Swap bytes within a 16-bit word: 0xAABB -> 0xBBAA
 */
export function swapBytes16(v: number): number {
  return ((v & 0xFF) << 8) | ((v >> 8) & 0xFF)
}

/**
 * Convert two 16-bit registers to a Float32 string.
 */
export function toFloat32(hi: number, lo: number): string {
  const buf = new ArrayBuffer(4)
  const view = new DataView(buf)
  view.setUint16(0, hi)
  view.setUint16(2, lo)
  return view.getFloat32(0).toPrecision(7)
}

/**
 * 16-bit value formatting composable.
 */
export function use16BitFormat(rawValue: Ref<number>) {
  const signed16 = computed(() => {
    const v = rawValue.value & 0xFFFF
    return v >= 0x8000 ? v - 0x10000 : v
  })

  const unsigned16 = computed(() => {
    return rawValue.value & 0xFFFF
  })

  const hex16 = computed(() => {
    return '0x' + (rawValue.value & 0xFFFF).toString(16).toUpperCase().padStart(4, '0')
  })

  const binary16 = computed(() => {
    const b = (rawValue.value & 0xFFFF).toString(2).padStart(16, '0')
    return `${b.slice(0, 4)} ${b.slice(4, 8)} ${b.slice(8, 12)} ${b.slice(12, 16)}`
  })

  return { signed16, unsigned16, hex16, binary16 }
}

/**
 * 32-bit value formatting composable.
 */
export function use32BitFormat(hi: Ref<number>, lo: Ref<number>, enabled: Ref<boolean>) {
  const longABCD = computed(() => {
    if (!enabled.value) return '-'
    return (((hi.value << 16) | lo.value) >>> 0).toString()
  })

  const longCDAB = computed(() => {
    if (!enabled.value) return '-'
    return (((lo.value << 16) | hi.value) >>> 0).toString()
  })

  const longBADC = computed(() => {
    if (!enabled.value) return '-'
    return (((swapBytes16(hi.value) << 16) | swapBytes16(lo.value)) >>> 0).toString()
  })

  const longDCBA = computed(() => {
    if (!enabled.value) return '-'
    return (((swapBytes16(lo.value) << 16) | swapBytes16(hi.value)) >>> 0).toString()
  })

  const floatABCD = computed(() => {
    if (!enabled.value) return '-'
    return toFloat32(hi.value, lo.value)
  })

  const floatCDAB = computed(() => {
    if (!enabled.value) return '-'
    return toFloat32(lo.value, hi.value)
  })

  const floatBADC = computed(() => {
    if (!enabled.value) return '-'
    return toFloat32(swapBytes16(hi.value), swapBytes16(lo.value))
  })

  const floatDCBA = computed(() => {
    if (!enabled.value) return '-'
    return toFloat32(swapBytes16(lo.value), swapBytes16(hi.value))
  })

  return { longABCD, longCDAB, longBADC, longDCBA, floatABCD, floatCDAB, floatBADC, floatDCBA }
}

/**
 * 64-bit (Float64) value formatting composable.
 */
export function use64BitFormat(values: Ref<number[]>, enabled: Ref<boolean>) {
  function makeDouble(reorder: (i: number) => number, byteSwap: boolean): string {
    const buf = new ArrayBuffer(8)
    const view = new DataView(buf)
    for (let i = 0; i < 4; i++) {
      const v = values.value[reorder(i)] & 0xFFFF
      view.setUint16(i * 2, byteSwap ? swapBytes16(v) : v)
    }
    return view.getFloat64(0).toPrecision(15)
  }

  const doubleValue = computed(() => {
    if (!enabled.value) return '-'
    return makeDouble(i => i, false)
  })

  const doubleReversed = computed(() => {
    if (!enabled.value) return '-'
    return makeDouble(i => 3 - i, false)
  })

  const doubleByteSwap = computed(() => {
    if (!enabled.value) return '-'
    return makeDouble(i => i, true)
  })

  const doubleLittleEndian = computed(() => {
    if (!enabled.value) return '-'
    return makeDouble(i => 3 - i, true)
  })

  return { doubleValue, doubleReversed, doubleByteSwap, doubleLittleEndian }
}

export type ByteOrder = 'ABCD' | 'CDAB' | 'BADC' | 'DCBA'

/**
 * Encode a Float32 value into a pair of u16 registers with the specified byte order.
 */
export function float32ToU16Pair(value: number, byteOrder: ByteOrder): [number, number] {
  const buf = new ArrayBuffer(4)
  const view = new DataView(buf)
  view.setFloat32(0, value)
  const w0 = view.getUint16(0)
  const w1 = view.getUint16(2)
  const map: Record<ByteOrder, [number, number]> = {
    ABCD: [w0, w1],
    CDAB: [w1, w0],
    BADC: [swapBytes16(w0), swapBytes16(w1)],
    DCBA: [swapBytes16(w1), swapBytes16(w0)],
  }
  return map[byteOrder]
}

export type RegisterWriteType = 'raw' | 'uint16' | 'int16' | 'uint32' | 'int32' | 'uint64' | 'int64' | 'float32' | 'float64'

export function registerWriteWidth(type: RegisterWriteType): number {
  if (type.endsWith('64')) return 4
  if (type.endsWith('32')) return 2
  return 1
}

/** Letters describe the transmitted byte sequence of one value, high byte first. */
export function registerWriteOrders(type: RegisterWriteType): string[] {
  const width = registerWriteWidth(type)
  if (width === 1) return ['AB', 'BA']
  if (width === 2) return ['ABCD', 'CDAB', 'BADC', 'DCBA']
  return ['ABCDEFGH', 'GHEFCDAB', 'BADCFEHG', 'HGFEDCBA']
}

/** Parse text before encoding so 64-bit integers never pass through Number. */
export function encodeRegisterWrite(input: string, type: RegisterWriteType, order: string): number[] {
  if (!registerWriteOrders(type).includes(order)) throw new Error('Invalid byte order')
  const text = input.trim()
  const width = registerWriteWidth(type)
  const view = new DataView(new ArrayBuffer(width * 2))
  if (type === 'float32' || type === 'float64') {
    if (!/^[+-]?(?:\d+(?:\.\d*)?|\.\d+)(?:e[+-]?\d+)?$/i.test(text)) throw new Error('Invalid float')
    const value = Number(text)
    if (!Number.isFinite(value)) throw new Error('Float overflow')
    if (type === 'float32') {
      if (!Number.isFinite(Math.fround(value))) throw new Error('Float32 overflow')
      view.setFloat32(0, value)
    } else view.setFloat64(0, value)
  } else {
    if (!/^(?:[+-]?\d+|0x[\da-f]+)$/i.test(text)) throw new Error('Invalid integer')
    const value = BigInt(text)
    const bits = BigInt(width * 16)
    const signed = type.startsWith('int')
    const min = signed ? -(1n << (bits - 1n)) : 0n
    const max = signed ? (1n << (bits - 1n)) - 1n : (1n << bits) - 1n
    if (value < min || value > max) throw new Error('Integer out of range')
    if (width === 4) view.setBigUint64(0, BigInt.asUintN(64, value))
    else if (width === 2) view.setUint32(0, Number(BigInt.asUintN(32, value)))
    else view.setUint16(0, Number(BigInt.asUintN(16, value)))
  }
  const bytes = [...order].map(letter => view.getUint8(letter.charCodeAt(0) - 65))
  return Array.from({ length: width }, (_, index) => bytes[index * 2]! * 256 + bytes[index * 2 + 1]!)
}
