<script setup lang="ts">
import { vModal } from "simlab-ui/modal"
import { ref, watch, computed, onBeforeUnmount } from 'vue'
import { invoke } from 'simlab-ui/configInvoke'
import type { CommandType, ControlResult } from '../types'
import { useI18n } from '@shared/i18n'
import AppButton from '@shared/components/ui/AppButton.vue'
import AppSelect from '@shared/components/ui/AppSelect.vue'
import { controlResultStatus } from '../utils/controlResult'

const { t } = useI18n()

interface Props {
  visible: boolean
  connectionId: string | null
  commonAddress: number
  prefillIoa?: number | null
  prefillCommandType?: CommandType | null
}

const props = defineProps<Props>()
const emit = defineEmits<{
  (e: 'close'): void
  (e: 'sent'): void
}>()

// Persist user-entered fields across opens AND across app restarts. The
// values reset *only* when the caller explicitly prefills them (clicking
// a specific data point), or on first launch.
const STORAGE_KEY = 'iec104master.controlDialog.v1'
type Persisted = {
  ca: number
  ioa: number
  commandType: CommandType
  controlMode: 'execute' | 'select' | 'sbo'
  singleValue: string
  doubleValue: string
  stepValue: string
  normalizedValue: string
  scaledValue: string
  floatValue: string
  bitstringValue: number
}
function loadPersisted(): Partial<Persisted> {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (raw) return JSON.parse(raw) as Partial<Persisted>
  } catch { /* ignore */ }
  return {}
}
const saved = loadPersisted()

const ioa = ref<number>(saved.ioa ?? 0)
const ca = ref<number>(saved.ca ?? 1)
const commandType = ref<CommandType>(saved.commandType ?? 'single')
// 兼容旧持久化字段 selectMode:true→'sbo',其余→'execute'。
const legacySelectMode = (saved as { selectMode?: boolean }).selectMode
const controlMode = ref<'execute' | 'select' | 'sbo'>(
  saved.controlMode ?? (legacySelectMode ? 'sbo' : 'execute'),
)
// 挂载时若命令类型为位串,强制控制模式为仅执行(位串 C_BO_NA_1 无 S/E 位)
if (commandType.value === 'bitstring' && controlMode.value !== 'execute') {
  controlMode.value = 'execute'
}
const errorMsg = ref('')
const sending = ref(false)
const lastResult = ref<ControlResult | null>(null)
const resultStatus = computed(() => lastResult.value ? controlResultStatus(lastResult.value) : null)
const waitingForConfirmation = ref(false)
let requestGeneration = 0

function resetRequest() {
  requestGeneration++
  sending.value = false
  waitingForConfirmation.value = false
  lastResult.value = null
  errorMsg.value = ''
}

function close() {
  resetRequest()
  emit('close')
}

onBeforeUnmount(resetRequest)

// Advanced parameters (intentionally not persisted — these are per-send
// tweaks the user usually wants to set deliberately each time)
const showAdvanced = ref(false)
const qualifier = ref<number>(0)
const cot = ref<number>(6)

// Value state per type — persisted so the user's last-typed value for
// each command type comes back the next time they open the dialog.
const singleValue = ref(saved.singleValue ?? 'true')
const doubleValue = ref(saved.doubleValue ?? '2')
const stepValue = ref(saved.stepValue ?? '2')
// 旧版本持久化的是 [-1,1) 小数；现在统一存原始 NVA 整数字符串。
// 加载时取整并夹到 i16 范围，避免历史小数残留导致解析失败。
function toRawNorm(x: string | undefined): string {
  const n = Number(x)
  if (x == null || x === '' || !Number.isFinite(n)) return '0'
  return String(Math.max(-32768, Math.min(32767, Math.round(n))))
}
const normalizedValue = ref(toRawNorm(saved.normalizedValue))
const scaledValue = ref(saved.scaledValue ?? '0')
const floatValue = ref(saved.floatValue ?? '0.0')
const bitstringValue = ref<number>(saved.bitstringValue ?? 0)

function persistedValues(): Persisted {
  return {
    ca: ca.value,
    ioa: ioa.value,
    commandType: commandType.value,
    controlMode: controlMode.value,
    singleValue: singleValue.value,
    doubleValue: doubleValue.value,
    stepValue: stepValue.value,
    normalizedValue: normalizedValue.value,
    scaledValue: scaledValue.value,
    floatValue: floatValue.value,
    bitstringValue: bitstringValue.value,
  }
}

watch(() => [props.visible, props.connectionId] as const, ([v]) => {
  resetRequest()
  if (v) {
    qualifier.value = 0
    cot.value = 6
    showAdvanced.value = false
    // Only override CA / IOA / commandType when the caller explicitly asked
    // for a specific point (right-click on a data row). Toolbar's "custom
    // control" passes prefillIoa = null, so the previously-saved values
    // — including the CA the user typed last time — are preserved.
    if (props.prefillIoa != null) ioa.value = props.prefillIoa
    if (props.prefillCommandType) commandType.value = props.prefillCommandType
  }
}, { flush: 'sync' })

watch(commandType, () => {
  qualifier.value = 0
  // 位串 C_BO_NA_1 无 S/E 位,只能仅执行
  if (commandType.value === 'bitstring' && controlMode.value !== 'execute') {
    controlMode.value = 'execute'
  }
})

const currentValueStr = computed(() => {
  // Coerce to string in every branch — backend expects `value: String` and
  // Vue's v-model on <input type="number"> can yield a JS number depending
  // on the entered text (e.g. typing "123" into a setpoint field), which
  // then fails serde deserialization with
  //   invalid type: integer `123`, expected a string
  switch (commandType.value) {
    case 'single': return String(singleValue.value)
    case 'double': return String(doubleValue.value)
    case 'step': return String(stepValue.value)
    case 'setpoint_normalized': return String(normalizedValue.value)
    case 'setpoint_scaled': return String(scaledValue.value)
    case 'setpoint_float': return String(floatValue.value)
    case 'bitstring': return String(bitstringValue.value)
  }
})

const qualifierHint = computed(() => {
  switch (commandType.value) {
    case 'single':
    case 'double':
    case 'step':
      return t('control.qulqlSingle')
    case 'setpoint_normalized':
    case 'setpoint_scaled':
    case 'setpoint_float':
      return t('control.qulqlSetpoint')
    case 'bitstring':
      return t('control.qulqlBitstring')
  }
})

const isBitstring = computed(() => commandType.value === 'bitstring')

const commandTypes = computed<{ value: CommandType; label: string }[]>(() => [
  { value: 'single', label: t('control.cmdSingle') },
  { value: 'double', label: t('control.cmdDouble') },
  { value: 'step', label: t('control.cmdStep') },
  { value: 'setpoint_normalized', label: t('control.cmdSetNorm') },
  { value: 'setpoint_scaled', label: t('control.cmdSetScaled') },
  { value: 'setpoint_float', label: t('control.cmdSetFloat') },
  { value: 'bitstring', label: t('control.cmdBitstring') },
])

// AppSelect 的 model 是 string；这两处包装负责与联合类型 ref 互转。
const commandTypeValue = computed({
  get: () => commandType.value,
  set: (v: string) => { commandType.value = v as CommandType },
})

const controlModeOptions = computed(() => [
  { value: 'execute', label: t('control.modeExecute') },
  { value: 'select', label: t('control.modeSelect') },
  { value: 'sbo', label: t('control.modeSbo') },
])
const controlModeValue = computed({
  get: () => controlMode.value,
  set: (v: string) => { controlMode.value = v as typeof controlMode.value },
})

async function send() {
  if (!props.visible || !props.connectionId || sending.value) return
  const generation = ++requestGeneration
  const connectionId = props.connectionId
  const savedValues = persistedValues()
  const isCurrentRequest = () => generation === requestGeneration && props.visible && props.connectionId === connectionId
  errorMsg.value = ''
  sending.value = true
  waitingForConfirmation.value = controlMode.value === 'sbo' || [6, 8].includes(cot.value & 0x3f)
  lastResult.value = null

  try {
    const payload: Record<string, unknown> = {
      connection_id: connectionId,
      ioa: ioa.value,
      common_address: ca.value,
      command_type: commandType.value,
      value: currentValueStr.value,
      control_mode: controlMode.value,
      select: controlMode.value === 'sbo', // 兼容后端旧字段
      qualifier: qualifier.value,
      cot: cot.value,
    }
    if (isBitstring.value) {
      payload.bitstring = bitstringValue.value >>> 0
    }
    const result = await invoke<ControlResult>('send_control_command', { request: payload })
    if (!isCurrentRequest()) return
    lastResult.value = result
    // Save the values actually sent, even if the dialog's context later changes.
    try { localStorage.setItem(STORAGE_KEY, JSON.stringify(savedValues)) } catch { /* ignore */ }
    emit('sent')
  } catch (e) {
    if (!isCurrentRequest()) return
    errorMsg.value = String(e)
  } finally {
    if (isCurrentRequest()) {
      sending.value = false
      waitingForConfirmation.value = false
    }
  }
}

function handleKeydown(e: KeyboardEvent) {
  if (e.key === 'Enter' && !e.isComposing && e.keyCode !== 229 && e.target instanceof HTMLInputElement && !['checkbox', 'radio', 'button', 'submit'].includes(e.target.type)) {
    e.preventDefault()
    send()
  }
}

// Pull the configured Common Addresses for the current connection so the
// CA field can offer a dropdown instead of forcing free-form numeric
// entry. Cached per-connection for the lifetime of the dialog session;
// re-queried whenever the dialog is opened against a different connection
// (the user might have created or edited a connection in between).
const availableCAs = ref<number[]>([])
async function loadAvailableCAs() {
  if (!props.connectionId) { availableCAs.value = []; return }
  try {
    const conns = await invoke<{ id: string; common_addresses: number[] }[]>('list_connections')
    const conn = conns.find((c) => c.id === props.connectionId)
    availableCAs.value = conn?.common_addresses?.slice() ?? []
  } catch {
    availableCAs.value = []
  }
}
watch(() => [props.visible, props.connectionId] as const, ([v]) => {
  if (v) loadAvailableCAs()
})

// Show the CA as a `<select>` only when the connection has more than one
// configured CA. Single-CA setups stay with the simpler number input.
const useCASelect = computed(() => availableCAs.value.length > 1)

// Special sentinel for "I want to type a CA that's not in the list".
const CUSTOM_CA = -1

// What `<option value>` to show as selected. If the persisted CA isn't in
// the list, we pin the dropdown to "Custom" so the number input appears.
const caSelectValue = computed<number>({
  get: () => availableCAs.value.includes(ca.value) ? ca.value : CUSTOM_CA,
  set: (v: number) => {
    if (v === CUSTOM_CA) {
      // Switching to custom — leave ca.value as-is so the user keeps editing it
      return
    }
    ca.value = v
  },
})
</script>

<template>
  <Teleport to="body">
    <Transition name="dialog-pop">
    <div v-if="visible" v-modal="close" class="modal-backdrop dialog-blur" @mousedown.self="close" @keydown="handleKeydown">
      <div class="modal-box">
        <div class="modal-title">{{ t('control.title') }}</div>
        <div class="modal-body">
          <fieldset class="form-fields" :disabled="sending">
          <div class="form-row">
            <label class="address-label ca-label" for="control-ca">{{ t('control.targetCa') }}</label>
            <label class="address-label ioa-label" for="control-ioa">{{ t('control.ioa') }}</label>
            <div class="ca-controls">
              <template v-if="useCASelect">
                <select id="control-ca" v-model.number="caSelectValue" class="form-input">
                  <option v-for="opt in availableCAs" :key="opt" :value="opt">CA {{ opt }}</option>
                  <option :value="CUSTOM_CA">{{ t('control.caCustom') }}</option>
                </select>
                <input
                  v-if="caSelectValue === CUSTOM_CA"
                  v-model.number="ca"
                  :aria-label="t('control.targetCa')"
                  class="form-input"
                  type="number"
                  min="1"
                  max="65534"
                />
              </template>
              <input v-else id="control-ca" v-model.number="ca" class="form-input" type="number" min="1" max="65534" />
            </div>
            <input id="control-ioa" v-model.number="ioa" class="form-input ioa-input" type="number" min="0" max="16777215" />
          </div>

          <label class="form-label">
            {{ t('control.commandType') }}
            <AppSelect v-model="commandTypeValue" :options="commandTypes" />
          </label>

          <!-- Single point: toggle -->
          <div v-if="commandType === 'single'" class="ctrl-buttons">
            <button :class="['ctrl-btn', { active: singleValue === 'false' }]" @click="singleValue = 'false'">{{ t('control.optOff') }}</button>
            <button :class="['ctrl-btn', { active: singleValue === 'true' }]" @click="singleValue = 'true'">{{ t('control.optOn') }}</button>
          </div>

          <!-- Double point: 4 buttons -->
          <div v-else-if="commandType === 'double'" class="ctrl-buttons">
            <button :class="['ctrl-btn ctrl-btn-sm', { active: doubleValue === '0' }]" @click="doubleValue = '0'">{{ t('control.optIntermediate') }}</button>
            <button :class="['ctrl-btn ctrl-btn-sm', { active: doubleValue === '1' }]" @click="doubleValue = '1'">{{ t('control.optOpen') }}</button>
            <button :class="['ctrl-btn ctrl-btn-sm', { active: doubleValue === '2' }]" @click="doubleValue = '2'">{{ t('control.optClose') }}</button>
            <button :class="['ctrl-btn ctrl-btn-sm', { active: doubleValue === '3' }]" @click="doubleValue = '3'">{{ t('control.optInvalid') }}</button>
          </div>

          <!-- Step: up/down -->
          <div v-else-if="commandType === 'step'" class="ctrl-buttons">
            <button :class="['ctrl-btn', { active: stepValue === '1' }]" @click="stepValue = '1'">&#9660; {{ t('control.optStepDown') }}</button>
            <button :class="['ctrl-btn', { active: stepValue === '2' }]" @click="stepValue = '2'">&#9650; {{ t('control.optStepUp') }}</button>
          </div>

          <!-- Normalized: raw NVA integer (-32768 ~ 32767), same form as scaled -->
          <label v-else-if="commandType === 'setpoint_normalized'" class="form-label">
            {{ t('control.valueRangeNormalized') }}
            <input v-model="normalizedValue" class="form-input" type="number" min="-32768" max="32767" step="1" />
          </label>

          <!-- Scaled: integer input -->
          <label v-else-if="commandType === 'setpoint_scaled'" class="form-label">
            {{ t('control.valueRangeScaled') }}
            <input v-model="scaledValue" class="form-input" type="number" min="-32768" max="32767" step="1" />
          </label>

          <!-- Float: number input -->
          <label v-else-if="commandType === 'setpoint_float'" class="form-label">
            {{ t('control.valueLabel') }}
            <input v-model="floatValue" class="form-input" type="number" step="0.1" />
          </label>

          <!-- Bitstring: 32-bit unsigned -->
          <div v-else-if="commandType === 'bitstring'" class="bitstring-control">
            <label class="form-label">
              {{ t('control.valueRangeBitstring') }}
              <input v-model.number="bitstringValue" class="form-input" type="number" min="0" :max="0xFFFFFFFF" step="1" />
            </label>
            <div class="bitstring-hex">{{ t('control.bitstringHex') }}: 0x{{ ((bitstringValue >>> 0).toString(16).toUpperCase().padStart(8, '0')) }}</div>
          </div>

          <div class="toggle-row">
            <label class="toggle-label">
              <span>{{ t('control.controlMode') }}</span>
              <AppSelect v-model="controlModeValue" class="mode-select" :options="controlModeOptions" :disabled="isBitstring" />
            </label>
            <span class="toggle-hint">{{ isBitstring ? t('control.bitstringNoSbo') : (controlMode === 'sbo' ? t('control.sboTwoStep') : (controlMode === 'select' ? t('control.modeSelectHint') : t('control.sboDirect'))) }}</span>
          </div>

          <details class="advanced" :open="showAdvanced" @toggle="showAdvanced = ($event.target as HTMLDetailsElement).open">
            <summary class="advanced-summary">{{ t('control.advancedSummary') }}</summary>
            <div class="advanced-body">
              <label class="form-label">
                {{ isBitstring ? t('control.qulqlIgnored') : t('control.qulqlLabel') }}
                <input v-model.number="qualifier" class="form-input" type="number" min="0" max="127" :disabled="isBitstring" />
                <span class="hint">{{ qualifierHint }}</span>
              </label>
              <label class="form-label">
                {{ t('control.cotLabel') }}
                <div class="cot-row">
                  <input v-model.number="cot" class="form-input cot-input" type="number" min="0" max="63" />
                  <select class="form-input cot-preset" @change="cot = Number(($event.target as HTMLSelectElement).value)">
                    <option value="6">{{ t('control.cot6') }}</option>
                    <option value="7">{{ t('control.cot7') }}</option>
                    <option value="8">{{ t('control.cot8') }}</option>
                    <option value="9">{{ t('control.cot9') }}</option>
                    <option value="10">{{ t('control.cot10') }}</option>
                  </select>
                </div>
              </label>
            </div>
          </details>
          </fieldset>

          <div v-if="errorMsg" class="error-msg" role="alert">{{ errorMsg }}</div>

          <div v-if="sending" class="result-indicator result-pending" role="status">
            {{ waitingForConfirmation ? t('control.waitingForConfirmation') : t('control.sending') }}
          </div>

          <div v-if="lastResult && resultStatus" :class="['result-indicator', resultStatus.confirmed ? 'result-ok' : 'result-unconfirmed']" role="status">
            <span class="result-steps">
              <span v-for="(step, i) in lastResult.steps" :key="i" class="step-dot" :title="step.action">&#9679;</span>
            </span>
            <span class="result-text">{{ t(resultStatus.label) }} · {{ t(resultStatus.duration, { ms: lastResult.duration_ms }) }}</span>
          </div>
        </div>
        <div class="modal-footer">
          <AppButton @click="close">{{ t('common.close') }}</AppButton>
          <AppButton variant="primary" :disabled="sending || !connectionId" @click="send">
            {{ sending ? (waitingForConfirmation ? t('control.waitingForConfirmation') : t('control.sending')) : t('control.send') }}
          </AppButton>
        </div>
      </div>
    </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.modal-backdrop {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.55);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}

.modal-box {
  background: var(--bg-app);
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-lg);
  padding: 20px;
  width: 520px;
  max-width: calc(100vw - 32px);
  /* issue #28:setpoint + 展开高级区 + 错误/结果提示时约 500-560px,小视口下标题/按钮
     必须常驻,滚动收进 .modal-body —— 之前连 max-height 都没有,超高内容直接被裁。 */
  max-height: 90vh;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  box-shadow: var(--shadow-lg);
}

.modal-title {
  flex-shrink: 0;
  font-size: var(--text-lg);
  font-weight: 600;
  color: var(--text-primary);
  margin-bottom: 16px;
}

.modal-body {
  display: flex;
  flex-direction: column;
  gap: 12px;
  flex: 1 1 auto;
  min-height: 0;
  overflow-y: auto;
}

.modal-footer {
  flex-shrink: 0;
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 20px;
}

.form-fields {
  display: flex;
  flex-direction: column;
  gap: 12px;
  min-width: 0;
  margin: 0;
  padding: 0;
  border: 0;
}

.form-label {
  display: flex;
  flex-direction: column;
  gap: 4px;
  font-size: 12px;
  color: var(--text-muted);
}

.form-row {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  grid-template-areas: "ca-label ioa-label" "ca-control ioa-control";
  align-items: start;
  gap: 4px 12px;
}

.address-label {
  min-width: 0;
  font-size: 12px;
  line-height: 1.4;
  color: var(--text-muted);
}
.ca-label { grid-area: ca-label; }
.ioa-label { grid-area: ioa-label; }
.ca-controls {
  grid-area: ca-control;
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-width: 0;
}
.ioa-input { grid-area: ioa-control; }
.form-row .form-input {
  width: 100%;
  min-width: 0;
  height: 32px;
}

@media (max-width: 480px) {
  .form-row {
    grid-template-columns: minmax(0, 1fr);
    grid-template-areas: "ca-label" "ca-control" "ioa-label" "ioa-control";
  }
  .ioa-label { margin-top: 8px; }
  .toggle-row { align-items: flex-start; flex-direction: column; gap: 6px; }
}

.bitstring-control {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.bitstring-hex {
  font-family: var(--font-mono);
  font-size: 11px;
  color: var(--accent);
  padding-left: 2px;
}

.advanced {
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  padding: 6px 10px;
}

.advanced-summary {
  cursor: pointer;
  font-size: 12px;
  color: var(--text-primary);
  user-select: none;
}

.advanced-summary:hover {
  color: var(--accent);
}

.advanced-body {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding-top: 10px;
}

.hint {
  font-size: 10px;
  color: var(--text-muted);
  line-height: 1.4;
}

.cot-row {
  display: flex;
  gap: 6px;
}

.cot-input {
  width: 80px;
}

.cot-preset {
  flex: 1;
}

/* 数字输入与 CA 下拉保持原生（v-model.number + 数字哨兵值），观感对齐 AppInput。 */
.form-input {
  padding: 6px 10px;
  background: var(--bg-panel);
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-md);
  color: var(--text-primary);
  font-size: var(--text-md);
}

.form-input:focus {
  outline: none;
  border-color: var(--accent);
  box-shadow: 0 0 0 2px color-mix(in srgb, var(--accent) 30%, transparent);
}

.ctrl-buttons {
  display: flex;
  gap: 6px;
}

.ctrl-btn {
  flex: 1;
  padding: 8px 12px;
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-md);
  background: var(--bg-raised);
  color: var(--text-primary);
  font-size: 12px;
  cursor: pointer;
  transition: background var(--duration-fast) var(--ease-out),
    border-color var(--duration-fast) var(--ease-out);
}

.ctrl-btn:hover {
  background: var(--bg-active);
}

.ctrl-btn.active {
  background: var(--accent);
  color: var(--on-accent);
  border-color: var(--accent);
  font-weight: 600;
}

.ctrl-btn-sm {
  padding: 6px 8px;
  font-size: 11px;
}

.toggle-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 4px 0;
}

.toggle-label {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  color: var(--text-primary);
  cursor: pointer;
}

/* AppSelect 自带主题化外观；这里只约束内联宽度。 */
.mode-select {
  width: 150px;
  min-width: 150px;
}

.toggle-hint {
  font-size: 10px;
  color: var(--text-muted);
}

.error-msg {
  padding: 8px 10px;
  background: color-mix(in srgb, var(--danger) 15%, transparent);
  border: 1px solid var(--danger);
  border-radius: var(--radius-sm);
  color: var(--danger);
  font-size: 12px;
  word-break: break-word;
}

.result-indicator {
  padding: 6px 8px;
  border-radius: var(--radius-sm);
  font-size: 11px;
  display: flex;
  align-items: center;
  gap: 8px;
}

.result-ok {
  background: color-mix(in srgb, var(--success) 15%, transparent);
  border: 1px solid color-mix(in srgb, var(--success) 30%, transparent);
  color: var(--success);
}

.result-unconfirmed,
.result-pending {
  background: var(--bg-panel);
  border: 1px solid var(--border-subtle);
  color: var(--text-muted);
}

.result-steps {
  display: flex;
  gap: 4px;
  font-size: 8px;
}

.step-dot {
  color: currentColor;
}

.result-text {
  font-family: var(--font-mono);
}
</style>
