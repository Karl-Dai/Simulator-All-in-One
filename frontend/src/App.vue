<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, reactive, ref } from 'vue'
import * as api from './api'
import type {
  CounterView,
  LinkConfig,
  LogEntry,
  Snapshot,
  StartConfig,
  TransportConfig,
} from './types'
import type { UnlistenFn } from '@tauri-apps/api/event'

const LOG_LIMIT = 500

const LINK_STATE_LABELS: Record<string, string> = {
  reset: '复位',
  available: '可用',
  busy: '忙',
  idle: '空闲',
}

interface CounterRow {
  id: number
  ioa: number
  reading: number
}

let nextRowId = 0

function makeRow(ioa: number, reading: number): CounterRow {
  nextRowId += 1
  return { id: nextRowId, ioa, reading }
}

function defaultCounters(): CounterRow[] {
  return [
    makeRow(1, 100),
    makeRow(2, 200),
    makeRow(3, 300),
  ]
}

const snapshot = ref<Snapshot>({
  running: false,
  linkState: 'idle',
  counters: [],
  stats: { framesRx: 0, framesTx: 0, errors: 0 },
  logs: [],
})

const transport = reactive<TransportConfig>({
  mode: 'tcp',
  host: '127.0.0.1',
  port: 2402,
  serialPort: '',
  baudRate: 9600,
  dataBits: 8,
  parity: 'none',
  stopBits: 1,
})

const link = reactive<LinkConfig>({
  address: 1,
  addressLen: 1,
  commonAddress: 1,
  caLen: 1,
  ioaLen: 3,
  pollIntervalMs: 500,
})

const counters = ref<CounterRow[]>(defaultCounters())
const logs = ref<LogEntry[]>([])
const drafts = reactive<Partial<Record<number, string>>>({})
const importInput = ref<HTMLInputElement | null>(null)
const onlyErrors = ref(false)
const starting = ref(false)
const busy = ref(false)
const formError = ref('')
const opError = ref('')

const running = computed(() => snapshot.value.running)
const linkStateLabel = computed(
  () => LINK_STATE_LABELS[snapshot.value.linkState] ?? snapshot.value.linkState,
)
const visibleLogs = computed(() =>
  onlyErrors.value ? logs.value.filter((entry) => entry.direction === 'error') : logs.value,
)

function directionLabel(direction: string): string {
  if (direction === 'tx') return 'TX'
  if (direction === 'rx') return 'RX'
  if (direction === 'error') return 'ERROR'
  return 'INFO'
}

function isInteger(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value) && Number.isInteger(value)
}

function addressLimit(len: number): number {
  if (len === 1) return 0xff
  if (len === 2) return 0xffff
  if (len === 3) return 0xffffff
  return 0
}

function draftFor(counter: CounterView): string {
  const draft = drafts[counter.ioa]
  if (draft !== undefined) return draft
  return isInteger(counter.reading) ? String(counter.reading) : ''
}

function appendLogs(entries: LogEntry[]): void {
  if (entries.length === 0) return
  logs.value.push(...entries)
  if (logs.value.length > LOG_LIMIT) {
    logs.value.splice(0, logs.value.length - LOG_LIMIT)
  }
}

function clearLogs(): void {
  logs.value = []
}

function applySnapshot(next: Snapshot, withLogs: boolean): void {
  snapshot.value = {
    running: next.running,
    linkState: next.linkState,
    counters: next.counters,
    stats: next.stats,
    logs: [],
  }
  if (withLogs) {
    appendLogs(next.logs.slice(-LOG_LIMIT))
  }
}

/** 解析读数输入，非整数时返回 null。 */
function parseReading(raw: string): number | null {
  const text = raw.trim()
  if (text === '') return null
  const num = Number(text)
  if (!Number.isFinite(num) || !Number.isInteger(num)) return null
  return num
}

function validate(): string {
  if (transport.mode === 'tcp') {
    if (!transport.host.trim()) return '请填写 TCP 主机地址'
    if (!isInteger(transport.port) || transport.port < 1 || transport.port > 65535) {
      return 'TCP 端口必须是 1 到 65535 的整数'
    }
  } else {
    if (!transport.serialPort.trim()) return '请填写串口名称'
    if (!isInteger(transport.baudRate) || transport.baudRate < 1) return '波特率必须是正整数'
    if (!isInteger(transport.dataBits) || transport.dataBits < 5 || transport.dataBits > 8) {
      return '数据位必须是 5 到 8 的整数'
    }
    if (!isInteger(transport.stopBits) || (transport.stopBits !== 1 && transport.stopBits !== 2)) {
      return '停止位只能是 1 或 2'
    }
    if (transport.parity !== 'none' && transport.parity !== 'odd' && transport.parity !== 'even') {
      return '校验方式无效'
    }
  }

  const addressMax = addressLimit(link.addressLen)
  if (!isInteger(link.address) || link.address < 0 || link.address > addressMax) {
    return `链路地址必须是 0 到 ${addressMax} 的整数`
  }
  if (link.addressLen !== 1 && link.addressLen !== 2) return '链路地址长度只能是 1 或 2'

  const caMax = addressLimit(link.caLen)
  if (!isInteger(link.commonAddress) || link.commonAddress < 0 || link.commonAddress > caMax) {
    return `公共地址必须是 0 到 ${caMax} 的整数`
  }
  if (link.caLen !== 1 && link.caLen !== 2) return '公共地址长度只能是 1 或 2'

  if (!isInteger(link.ioaLen) || link.ioaLen < 1 || link.ioaLen > 3) {
    return '信息对象地址长度只能是 1、2 或 3'
  }
  if (!isInteger(link.pollIntervalMs) || link.pollIntervalMs < 1) {
    return '轮询间隔必须是正整数（毫秒）'
  }

  const ioaMax = addressLimit(link.ioaLen)
  const seen = new Set<number>()
  for (const row of counters.value) {
    if (!isInteger(row.ioa) || row.ioa < 0 || row.ioa > ioaMax) {
      return `计量点表 IOA 必须是 0 到 ${ioaMax} 的整数`
    }
    if (seen.has(row.ioa)) return `计量点表存在重复的 IOA=${row.ioa}`
    seen.add(row.ioa)
    if (!isInteger(row.reading)) return `IOA=${row.ioa} 读数必须是整数`
  }
  return ''
}

function buildConfig(): StartConfig {
  return {
    transport: { ...transport },
    link: { ...link },
    counters: counters.value.map((row) => ({
      ioa: Number(row.ioa),
      reading: Number(row.reading),
    })),
  }
}

function addCounter(): void {
  const used = new Set(
    counters.value.map((row) => Number(row.ioa)).filter((value) => Number.isFinite(value)),
  )
  let ioa = 1
  while (used.has(ioa)) ioa += 1
  counters.value.push(makeRow(ioa, 0))
}

function removeCounter(index: number): void {
  counters.value.splice(index, 1)
}

function exportCsv(): void {
  const lines = ['ioa,reading', ...counters.value.map((row) => `${row.ioa},${row.reading}`)]
  const blob = new Blob([`\ufeff${lines.join('\n')}\n`], { type: 'text/csv;charset=utf-8' })
  const url = URL.createObjectURL(blob)
  const anchor = document.createElement('a')
  anchor.href = url
  anchor.download = 'iec102-counters.csv'
  anchor.click()
  URL.revokeObjectURL(url)
}

function importCsv(event: Event): void {
  const input = event.target as HTMLInputElement
  const file = input.files?.[0]
  if (!file) return
  file.text().then((raw) => {
    const rows: CounterRow[] = []
    for (const line of raw.replace(/^\ufeff/, '').split(/\r?\n/)) {
      const trimmed = line.trim()
      if (!trimmed || /^ioa\s*,\s*reading/i.test(trimmed)) continue
      const [ioaText, readingText] = trimmed.split(',')
      const ioa = Number(ioaText)
      const reading = Number(readingText)
      if (!isInteger(ioa) || !isInteger(reading) || reading < 0) {
        formError.value = `CSV 行无效: ${trimmed}`
        return
      }
      rows.push(makeRow(ioa, reading))
    }
    if (rows.length === 0) {
      formError.value = 'CSV 没有有效计量点'
      return
    }
    counters.value = rows
    formError.value = ''
  })
  input.value = ''
}

function onInitReadingChange(row: CounterRow, event: Event): void {
  const raw = (event.target as HTMLInputElement).value
  const parsed = parseReading(raw)
  if (parsed === null) {
    formError.value = `IOA=${row.ioa} 读数必须是整数`
    return
  }
  row.reading = parsed
  formError.value = ''
}

function onDraft(counter: CounterView, event: Event): void {
  drafts[counter.ioa] = (event.target as HTMLInputElement).value
}

async function onCommit(counter: CounterView): Promise<void> {
  const raw = drafts[counter.ioa] ?? ''
  delete drafts[counter.ioa]
  const parsed = parseReading(raw)
  if (parsed === null) {
    opError.value = `IOA=${counter.ioa} 的读数无效`
    return
  }
  try {
    await api.setReading(counter.ioa, parsed)
  } catch (err) {
    const message = `设置 IOA=${counter.ioa} 失败：${String(err)}`
    opError.value = message
    appendLogs([api.makeErrorLog(message)])
  }
}

async function doStart(): Promise<void> {
  formError.value = ''
  const error = validate()
  if (error) {
    formError.value = error
    return
  }
  starting.value = true
  try {
    await api.start(buildConfig())
    applySnapshot(await api.snapshot(), false)
  } catch (err) {
    const message = `监听失败：${String(err)}`
    formError.value = message
    appendLogs([api.makeErrorLog(message)])
  } finally {
    starting.value = false
  }
}

async function doStop(): Promise<void> {
  starting.value = true
  try {
    await api.stop()
    applySnapshot(await api.snapshot(), false)
  } catch (err) {
    appendLogs([api.makeErrorLog(`停止失败：${String(err)}`)])
  } finally {
    starting.value = false
  }
}

async function doRefresh(): Promise<void> {
  opError.value = ''
  busy.value = true
  try {
    applySnapshot(await api.snapshot(), false)
  } catch (err) {
    const message = `刷新失败：${String(err)}`
    opError.value = message
    appendLogs([api.makeErrorLog(message)])
  } finally {
    busy.value = false
  }
}

let unlistenSnapshot: UnlistenFn | undefined
let unlistenLog: UnlistenFn | undefined

onMounted(async () => {
  try {
    applySnapshot(await api.snapshot(), true)
  } catch (err) {
    appendLogs([api.makeErrorLog(`读取快照失败：${String(err)}`)])
  }
  try {
    unlistenSnapshot = await api.onSnapshot((next) => applySnapshot(next, false))
    unlistenLog = await api.onLog((entry) => appendLogs([entry]))
  } catch (err) {
    appendLogs([api.makeErrorLog(`监听事件失败：${String(err)}`)])
  }
})

onBeforeUnmount(() => {
  unlistenSnapshot?.()
  unlistenLog?.()
})
</script>

<template>
  <div class="app">
    <header class="topbar">
      <div class="brand">
        <img src="/suite-icon.svg" alt="" class="brand-icon" />
        <span class="brand-title">IEC102 子站</span>
      </div>
      <div class="status">
        <span class="status-item">
          <span class="label">运行</span>
          <span class="value" :class="running ? 'ok' : 'muted'">
            {{ running ? '运行中' : '已停止' }}
          </span>
        </span>
        <span class="status-item">
          <span class="label">链路</span>
          <span class="value">{{ linkStateLabel }}</span>
        </span>
        <span class="status-item">
          <span class="label">接收帧</span>
          <span class="value">{{ snapshot.stats.framesRx }}</span>
        </span>
        <span class="status-item">
          <span class="label">发送帧</span>
          <span class="value">{{ snapshot.stats.framesTx }}</span>
        </span>
        <span class="status-item">
          <span class="label">错误</span>
          <span class="value" :class="{ err: snapshot.stats.errors > 0 }">
            {{ snapshot.stats.errors }}
          </span>
        </span>
      </div>
      <div class="topbar-actions">
        <button class="btn primary" :disabled="running || starting" @click="doStart">
          监听
        </button>
        <button class="btn" :disabled="!running || starting" @click="doStop">
          停止
        </button>
      </div>
    </header>

    <div class="body">
      <aside class="panel config">
        <h2>配置</h2>

        <div class="field">
          <label>传输模式</label>
          <div class="seg">
            <button
              type="button"
              :class="{ active: transport.mode === 'tcp' }"
              :disabled="running"
              @click="transport.mode = 'tcp'"
            >
              TCP 监听
            </button>
            <button
              type="button"
              :class="{ active: transport.mode === 'serial' }"
              :disabled="running"
              @click="transport.mode = 'serial'"
            >
              串口
            </button>
          </div>
        </div>

        <template v-if="transport.mode === 'tcp'">
          <div class="field">
            <label for="host">主机</label>
            <input id="host" v-model.trim="transport.host" :disabled="running" />
          </div>
          <div class="field">
            <label for="port">监听端口</label>
            <input
              id="port"
              v-model.number="transport.port"
              type="number"
              min="1"
              max="65535"
              :disabled="running"
            />
          </div>
        </template>

        <template v-else>
          <div class="field">
            <label for="serial-port">串口名称</label>
            <input
              id="serial-port"
              v-model.trim="transport.serialPort"
              placeholder="/dev/ttyUSB0"
              :disabled="running"
            />
          </div>
          <div class="field">
            <label for="baud">波特率</label>
            <input
              id="baud"
              v-model.number="transport.baudRate"
              type="number"
              min="1"
              :disabled="running"
            />
          </div>
          <div class="field-row">
            <div class="field">
              <label for="data-bits">数据位</label>
              <select id="data-bits" v-model.number="transport.dataBits" :disabled="running">
                <option v-for="n in [5, 6, 7, 8]" :key="n" :value="n">{{ n }}</option>
              </select>
            </div>
            <div class="field">
              <label for="parity">校验</label>
              <select id="parity" v-model="transport.parity" :disabled="running">
                <option value="none">无</option>
                <option value="odd">奇</option>
                <option value="even">偶</option>
              </select>
            </div>
            <div class="field">
              <label for="stop-bits">停止位</label>
              <select id="stop-bits" v-model.number="transport.stopBits" :disabled="running">
                <option :value="1">1</option>
                <option :value="2">2</option>
              </select>
            </div>
          </div>
        </template>

        <h2>链路</h2>

        <div class="field">
          <label for="link-address">链路地址</label>
          <input
            id="link-address"
            v-model.number="link.address"
            type="number"
            min="0"
            :disabled="running"
          />
        </div>
        <div class="field-row">
          <div class="field">
            <label for="address-len">链路地址长度</label>
            <select id="address-len" v-model.number="link.addressLen" :disabled="running">
              <option :value="1">1</option>
              <option :value="2">2</option>
            </select>
          </div>
          <div class="field">
            <label for="common-address">公共地址</label>
            <input
              id="common-address"
              v-model.number="link.commonAddress"
              type="number"
              min="0"
              :disabled="running"
            />
          </div>
        </div>
        <div class="field-row">
          <div class="field">
            <label for="ca-len">公共地址长度</label>
            <select id="ca-len" v-model.number="link.caLen" :disabled="running">
              <option :value="1">1</option>
              <option :value="2">2</option>
            </select>
          </div>
          <div class="field">
            <label for="ioa-len">信息对象地址长度</label>
            <select id="ioa-len" v-model.number="link.ioaLen" :disabled="running">
              <option :value="1">1</option>
              <option :value="2">2</option>
              <option :value="3">3</option>
            </select>
          </div>
        </div>
        <div class="field">
          <label for="poll">轮询间隔（毫秒）</label>
          <input
            id="poll"
            v-model.number="link.pollIntervalMs"
            type="number"
            min="1"
            :disabled="running"
          />
        </div>

        <h2>计量点表</h2>
        <div class="points-table">
          <table>
            <thead>
              <tr>
                <th>IOA</th>
                <th>读数</th>
                <th class="col-op"></th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="(row, index) in counters" :key="row.id">
                <td>
                  <input
                    v-model.number="row.ioa"
                    type="number"
                    min="0"
                    :disabled="running"
                  />
                </td>
                <td>
                  <input
                    v-model.number="row.reading"
                    type="number"
                    :step="1"
                    :disabled="running"
                    @change="onInitReadingChange(row, $event)"
                  />
                </td>
                <td class="col-op">
                  <button class="btn small" :disabled="running" @click="removeCounter(index)">
                    删除
                  </button>
                </td>
              </tr>
              <tr v-if="counters.length === 0">
                <td colspan="3" class="empty">计量点表为空</td>
              </tr>
            </tbody>
          </table>
          <div class="row-actions">
            <button class="btn small" :disabled="running" @click="addCounter">添加计量点</button>
            <button class="btn small" @click="exportCsv">导出 CSV</button>
            <button class="btn small" :disabled="running" @click="importInput?.click()">
              导入 CSV
            </button>
            <input
              ref="importInput"
              type="file"
              accept=".csv,text/csv"
              hidden
              @change="importCsv"
            />
          </div>
        </div>

        <p v-if="formError" class="form-error">{{ formError }}</p>
      </aside>

      <div class="main-col">
        <div class="panel ops">
          <button class="btn" :disabled="busy" @click="doRefresh">刷新</button>
          <span v-if="opError" class="op-error">{{ opError }}</span>
        </div>

        <div class="panel data">
          <h2>数据</h2>
          <div class="table-wrap">
            <table>
              <thead>
                <tr>
                  <th>计量点号</th>
                  <th>读数</th>
                  <th>顺序号</th>
                </tr>
              </thead>
              <tbody>
                <tr v-for="counter in snapshot.counters" :key="counter.ioa">
                  <td>{{ counter.ioa }}</td>
                  <td>
                    <template v-if="running">
                      <input
                        type="number"
                        class="value-input"
                        :step="1"
                        :value="draftFor(counter)"
                        @input="onDraft(counter, $event)"
                        @change="onCommit(counter)"
                      />
                    </template>
                    <template v-else>{{ counter.reading }}</template>
                  </td>
                  <td>{{ counter.sequence }}</td>
                </tr>
                <tr v-if="snapshot.counters.length === 0">
                  <td colspan="3" class="empty">尚未配置计量点表</td>
                </tr>
              </tbody>
            </table>
          </div>
        </div>
      </div>
    </div>

    <section class="panel logs">
      <div class="logs-head">
        <h2>日志</h2>
        <label class="chk">
          <input v-model="onlyErrors" type="checkbox" />
          只看错误
        </label>
        <button class="btn small" @click="clearLogs">清空</button>
      </div>
      <div class="logs-body">
        <div v-for="(entry, index) in visibleLogs" :key="index" class="log-line">
          <span class="log-at">{{ entry.at }}</span>
          <span class="badge" :class="entry.direction">{{ directionLabel(entry.direction) }}</span>
          <span class="log-summary">{{ entry.summary }}</span>
          <span v-if="entry.hex" class="log-hex">{{ entry.hex }}</span>
        </div>
        <div v-if="visibleLogs.length === 0" class="empty">暂无日志</div>
      </div>
    </section>
  </div>
</template>
