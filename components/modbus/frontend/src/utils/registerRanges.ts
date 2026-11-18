import type { appstate } from '../../wailsjs/go/models'

export type RegisterRange = appstate.RegisterRange
export const REGISTER_AREAS = [
  { type: 'coil', label: 'table.coil', fc: 'FC01 / FC05 / FC15', bit: true },
  { type: 'discrete_input', label: 'table.discreteInput', fc: 'FC02', bit: true },
  { type: 'holding_register', label: 'table.holdingRegister', fc: 'FC03 / FC06 / FC16', bit: false },
  { type: 'input_register', label: 'table.inputRegister', fc: 'FC04', bit: false },
] as const

export function rangeWidth(range: RegisterRange): number {
  return ['uint32', 'int32', 'float32'].includes(range.DataType) ? 2 : 1
}

export function rangeError(ranges: RegisterRange[], index: number): string | null {
  const r = ranges[index]!
  if (!Number.isInteger(r.StartAddress) || !Number.isInteger(r.EndAddress)
    || r.StartAddress < 0 || r.EndAddress > 65535 || r.StartAddress > r.EndAddress) return 'ranges.invalidAddress'
  const area = REGISTER_AREAS.find(a => a.type === r.RegisterType)
  if (!area || (area.bit ? r.DataType !== 'bool' : !['uint16', 'int16', 'uint32', 'int32', 'float32'].includes(r.DataType))
    || !['big', 'little', 'mid_big', 'mid_little'].includes(r.Endian)) return 'ranges.invalidType'
  if ((r.EndAddress - r.StartAddress + 1) % rangeWidth(r) !== 0) return 'ranges.incompletePoint'
  if (ranges.some((other, j) => j !== index && other.RegisterType === r.RegisterType
    && r.StartAddress <= other.EndAddress && other.StartAddress <= r.EndAddress)) return 'ranges.overlap'
  return null
}

export function rangesValid(ranges: RegisterRange[]): boolean {
  return ranges.every((_, index) => !rangeError(ranges, index))
}

export function rangeCounts(range: RegisterRange): { points: number; addresses: number } {
  const addresses = range.EndAddress - range.StartAddress + 1
  return { points: addresses / rangeWidth(range), addresses }
}
