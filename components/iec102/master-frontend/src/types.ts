export type TransportMode = 'tcp' | 'serial'
export type Parity = 'none' | 'odd' | 'even'
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

export interface ConnectConfig {
  transport: TransportConfig
  link: LinkConfig
}

export interface LogEntry {
  at: string
  direction: LogDirection
  hex: string
  summary: string
}

export interface CounterView {
  ioa: number
  reading: number
  sequence: number
}

export interface StatsView {
  framesRx: number
  framesTx: number
  errors: number
}

export interface Snapshot {
  connected: boolean
  linkState: LinkState
  counters: CounterView[]
  stats: StatsView
  logs: LogEntry[]
}
