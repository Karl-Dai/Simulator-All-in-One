export type TransportMode = 'tcp' | 'serial'
export type Parity = 'none' | 'odd' | 'even'
export type PointKind = 'single' | 'double' | 'normalized' | 'scaled' | 'float'
export type LinkState = 'reset' | 'available' | 'busy' | 'idle'
export type LogDirection = 'tx' | 'rx' | 'info' | 'error'

export interface TransportConfig {
  mode: TransportMode
  host: string
  port: number
  serialPort: string
  baudRate: number
  dataBits: number
  parity: Parity
  stopBits: number
}

export interface LinkConfig {
  address: number
  addressLen: number
  commonAddress: number
  caLen: number
  ioaLen: number
  pollIntervalMs: number
}

export interface PointInit {
  ioa: number
  kind: PointKind
  value: unknown
}

export interface StartConfig {
  transport: TransportConfig
  link: LinkConfig
  points: PointInit[]
}

export interface LogEntry {
  at: string
  direction: LogDirection
  hex: string
  summary: string
}

export interface PointView {
  ioa: number
  kind: string
  value: unknown
  quality: string
}

export interface StatsView {
  framesRx: number
  framesTx: number
  errors: number
}

export interface Snapshot {
  running: boolean
  linkState: LinkState
  points: PointView[]
  stats: StatsView
  logs: LogEntry[]
}
