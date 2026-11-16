<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, reactive, ref } from 'vue'
import { loadPersistedForm, watchPersistedForm } from './suite-persist'
import * as api from './api'
import type {
  ConnectConfig,
  LinkConfig,
  LogEntry,
  Snapshot,
  TransportConfig,
} from './types'
import type { UnlistenFn } from '@tauri-apps/api/event'

const LOG_LIMIT = 500

const KIND_LABELS: Record<string, string> = {
  single: '单点',
  double: '双点',
  normalized: '归一化',
  scaled: '标度化',
  float: '短浮点',
}

const LINK_STATE_LABELS: Record<string, string> = {
  reset: '复位',
  available: '可用',
  busy: '忙',
  idle: '空闲',
}

const snapshot = ref<Snapshot>({
  connected: false,
  linkState: 'idle',
  points: [],
  stats: { framesRx: 0, framesTx: 0, errors: 0 },
  logs: [],
})

const transport = reactive<TransportConfig>({
  mode: 'tcp',
  host: '127.0.0.1',
  port: 2401,
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

// 套件模式：恢复上次保存的连接参数，并在变化后写回。
if (import.meta.env.VITE_SIMLAB_SUITE === '1') {
  loadPersistedForm([transport, link])
  watchPersistedForm([transport, link])
}

const logs = ref<LogEntry[]>([])
const onlyErrors = ref(false)
const connecting = ref(false)
const busy = ref(false)
const formError = ref('')
const opError = ref('')
const controlIoa = ref<number | null>(null)

const connected = computed(() => snapshot.value.connected)
const linkStateLabel = computed(
  () => LINK_STATE_LABELS[snapshot.value.linkState] ?? snapshot.value.linkState,
)
const visibleLogs = computed(() =>
  onlyErrors.value ? logs.value.filter((entry) => entry.direction === 'error') : logs.value,
)

function kindLabel(kind: string): string {
  return KIND_LABELS[kind] ?? kind
}

function exportCsv(): void {
  const lines = [
    'ioa,kind,value',
    ...snapshot.value.points.map((point) => `${point.ioa},${point.kind},${valueLabel(point.value)}`),
  ]
  const blob = new Blob([`\ufeff${lines.join('\n')}\n`], { type: 'text/csv;charset=utf-8' })
  const url = URL.createObjectURL(blob)
  const anchor = document.createElement('a')
  anchor.href = url
  anchor.download = 'iec101-received.csv'
  anchor.click()
  URL.revokeObjectURL(url)
}

function valueLabel(value: unknown): string {
  if (typeof value === 'boolean') return value ? '合' : '分'
  if (typeof value === 'number') {
    return Number.isInteger(value) ? String(value) : value.toFixed(4)
  }
  if (value === null || value === undefined) return '—'
  return JSON.stringify(value)
}

function directionLabel(direction: string): string {
  if (direction === 'tx') return 'TX'
  if (direction === 'rx') return 'RX'
  if (direction === 'error') return 'ERROR'
  return 'INFO'
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
    connected: next.connected,
    linkState: next.linkState,
    points: next.points,
    stats: next.stats,
    logs: [],
  }
  if (withLogs) {
    appendLogs(next.logs.slice(-LOG_LIMIT))
  }
}

function isInteger(value: number | null): value is number {
  return typeof value === 'number' && Number.isFinite(value) && Number.isInteger(value)
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
    if (!['none', 'odd', 'even'].includes(transport.parity)) return '校验方式无效'
  }

  const addressMax = link.addressLen === 1 ? 255 : 65535
  if (!isInteger(link.address) || link.address < 0 || link.address > addressMax) {
    return `链路地址必须是 0 到 ${addressMax} 的整数`
  }
  if (link.addressLen !== 1 && link.addressLen !== 2) return '链路地址长度只能是 1 或 2'

  const caMax = link.caLen === 1 ? 255 : 65535
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
  return ''
}

function buildConfig(): ConnectConfig {
  return {
    transport: { ...transport },
    link: { ...link },
    points: [],
  }
}

async function doConnect(): Promise<void> {
  formError.value = ''
  const error = validate()
  if (error) {
    formError.value = error
    return
  }
  connecting.value = true
  try {
    await api.connect(buildConfig())
  } catch (err) {
    const message = `连接失败：${String(err)}`
    formError.value = message
    appendLogs([api.makeErrorLog(message)])
  } finally {
    connecting.value = false
  }
}

async function doDisconnect(): Promise<void> {
  try {
    await api.disconnect()
  } catch (err) {
    appendLogs([api.makeErrorLog(`断开失败：${String(err)}`)])
  }
}

async function doInterrogate(): Promise<void> {
  opError.value = ''
  busy.value = true
  try {
    await api.interrogate(20)
  } catch (err) {
    const message = `总召唤失败：${String(err)}`
    opError.value = message
    appendLogs([api.makeErrorLog(message)])
  } finally {
    busy.value = false
  }
}

function validateControlIoa(): number | null {
  const ioa = controlIoa.value
  if (!isInteger(ioa) || ioa < 1) {
    opError.value = '请输入有效的 IOA'
    return null
  }
  const max = Math.pow(256, link.ioaLen) - 1
  if (ioa > max) {
    opError.value = `IOA 超出范围，当前地址长度最大为 ${max}`
    return null
  }
  return ioa
}

async function doControl(on: boolean): Promise<void> {
  opError.value = ''
  const ioa = validateControlIoa()
  if (ioa === null) return
  busy.value = true
  try {
    await api.control(ioa, on)
  } catch (err) {
    const message = `遥控失败：${String(err)}`
    opError.value = message
    appendLogs([api.makeErrorLog(message)])
  } finally {
    busy.value = false
  }
}

async function doSyncClock(): Promise<void> {
  opError.value = ''
  busy.value = true
  try {
    await api.syncClock()
  } catch (err) {
    const message = `时钟同步失败：${String(err)}`
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
        <span class="brand-title">IEC101 主站</span>
      </div>
      <div class="status">
        <span class="status-item">
          <span class="label">连接</span>
          <span class="value" :class="connected ? 'ok' : 'muted'">
            {{ connected ? '已连接' : '未连接' }}
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
        <button class="btn primary" :disabled="connected || connecting" @click="doConnect">
          连接
        </button>
        <button class="btn" :disabled="!connected || connecting" @click="doDisconnect">
          断开
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
              :disabled="connected"
              @click="transport.mode = 'tcp'"
            >
              TCP
            </button>
            <button
              type="button"
              :class="{ active: transport.mode === 'serial' }"
              :disabled="connected"
              @click="transport.mode = 'serial'"
            >
              串口
            </button>
          </div>
        </div>

        <template v-if="transport.mode === 'tcp'">
          <div class="field">
            <label for="host">主机</label>
            <input id="host" v-model.trim="transport.host" :disabled="connected" />
          </div>
          <div class="field">
            <label for="port">端口</label>
            <input
              id="port"
              v-model.number="transport.port"
              type="number"
              min="1"
              max="65535"
              :disabled="connected"
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
              :disabled="connected"
            />
          </div>
          <div class="field">
            <label for="baud">波特率</label>
            <input
              id="baud"
              v-model.number="transport.baudRate"
              type="number"
              min="1"
              :disabled="connected"
            />
          </div>
          <div class="field-row">
            <div class="field">
              <label for="data-bits">数据位</label>
              <select id="data-bits" v-model.number="transport.dataBits" :disabled="connected">
                <option v-for="n in [5, 6, 7, 8]" :key="n" :value="n">{{ n }}</option>
              </select>
            </div>
            <div class="field">
              <label for="parity">校验</label>
              <select id="parity" v-model="transport.parity" :disabled="connected">
                <option value="none">无</option>
                <option value="odd">奇</option>
                <option value="even">偶</option>
              </select>
            </div>
            <div class="field">
              <label for="stop-bits">停止位</label>
              <select id="stop-bits" v-model.number="transport.stopBits" :disabled="connected">
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
            :disabled="connected"
          />
        </div>
        <div class="field-row">
          <div class="field">
            <label for="address-len">链路地址长度</label>
            <select id="address-len" v-model.number="link.addressLen" :disabled="connected">
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
              :disabled="connected"
            />
          </div>
        </div>
        <div class="field-row">
          <div class="field">
            <label for="ca-len">公共地址长度</label>
            <select id="ca-len" v-model.number="link.caLen" :disabled="connected">
              <option :value="1">1</option>
              <option :value="2">2</option>
            </select>
          </div>
          <div class="field">
            <label for="ioa-len">信息对象地址长度</label>
            <select id="ioa-len" v-model.number="link.ioaLen" :disabled="connected">
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
            :disabled="connected"
          />
        </div>

        <p v-if="formError" class="form-error">{{ formError }}</p>
      </aside>

      <div class="main-col">
        <div class="panel ops">
          <button class="btn primary" :disabled="!connected || busy" @click="doInterrogate">
            总召唤（QOI=20）
          </button>
          <div class="control-group">
            <input
              v-model.number="controlIoa"
              type="number"
              min="1"
              placeholder="IOA"
              class="ioa-input"
              :disabled="!connected || busy"
            />
            <button class="btn" :disabled="!connected || busy" @click="doControl(true)">合</button>
            <button class="btn" :disabled="!connected || busy" @click="doControl(false)">分</button>
          </div>
          <button class="btn" :disabled="!connected || busy" @click="doSyncClock">时钟同步</button>
          <span v-if="opError" class="op-error">{{ opError }}</span>
        </div>

        <div class="panel data">
          <h2>数据</h2>
          <button class="btn small" :disabled="snapshot.points.length === 0" @click="exportCsv">
            导出 CSV
          </button>
          <div class="table-wrap">
            <table>
              <thead>
                <tr>
                  <th>IOA</th>
                  <th>类型</th>
                  <th>值</th>
                  <th>品质</th>
                </tr>
              </thead>
              <tbody>
                <tr v-for="point in snapshot.points" :key="point.ioa">
                  <td>{{ point.ioa }}</td>
                  <td>{{ kindLabel(point.kind) }}</td>
                  <td>{{ valueLabel(point.value) }}</td>
                  <td>{{ point.quality || '—' }}</td>
                </tr>
                <tr v-if="snapshot.points.length === 0">
                  <td colspan="4" class="empty">尚未收到数据</td>
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
