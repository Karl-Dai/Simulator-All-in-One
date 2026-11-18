<script setup lang="ts">
import { ref, computed, inject, onMounted, onUnmounted, watch, type Ref } from 'vue'
import * as App from '../../wailsjs/go/main/App'
import { EventsOn } from '../../wailsjs/runtime/runtime'
import { useI18n, vModal } from 'shared-frontend'
import type { ScanGroupInfo, SlaveIdScanEvent, RegisterScanEvent, FoundRegisterDto } from '../types'
const { t } = useI18n()
const emit = defineEmits<{ (e: 'close'): void }>()
const selectedConnectionId = inject<Ref<string | null>>('selectedConnectionId')!
const selectedGroup = inject<Ref<ScanGroupInfo | null>>('selectedScanGroup', ref(null))
const refreshTree = inject<() => void>('refreshTree')!
const connectionID = selectedConnectionId.value!
const tab = ref<'slave' | 'register'>('slave')
const scanning = ref(false), cancelling = ref(false), adding = ref(false), loading = ref(true)
const autoAddGroups = ref(false), error = ref(''), addedMessage = ref('')
const slaveScanTimeout = ref(500), slaveStartId = ref(1), slaveEndId = ref(247)
const slaveProgress = ref({ CurrentID: 0, Completed: 0, Total: 247, FoundIDs: [] as number[] })
const slaveScanDone = ref(false), slaveCancelled = ref(false)
const regSlaveID = ref(selectedGroup.value?.SlaveID ?? 1), defaultSlaveID = ref(1)
const regFunction = ref(selectedGroup.value?.Function ?? 'read_holding_registers')
const regStartAddr = ref(selectedGroup.value?.StartAddress ?? 0), regEndAddr = ref(selectedGroup.value ? selectedGroup.value.StartAddress + selectedGroup.value.Quantity - 1 : 99)
const regChunkSize = ref(10), regTimeout = ref(1000)
const regProgress = ref({ CurrentAddress: 0, EndAddress: 0, Completed: 0, Total: 0, FoundRegisters: [] as FoundRegisterDto[] })
const regScanDone = ref(false), registerCancelled = ref(false), resultTarget = ref<{ slaveID: number; fn: string } | null>(null)
const regPage = ref(0), pageSize = 100
const regPages = computed(() => Math.max(1, Math.ceil(regProgress.value.FoundRegisters.length / pageSize)))
const visibleRegisters = computed(() => regProgress.value.FoundRegisters.slice(regPage.value * pageSize, (regPage.value + 1) * pageSize))
const busy = computed(() => scanning.value || adding.value || loading.value)
const maxChunk = computed(() => ['read_coils', 'read_discrete_inputs'].includes(regFunction.value) ? 2000 : 125)
const functionOptions = [ { value: 'read_coils', label: 'FC01 Read Coils' }, { value: 'read_discrete_inputs', label: 'FC02 Read Discrete Inputs' }, { value: 'read_holding_registers', label: 'FC03 Read Holding Registers' }, { value: 'read_input_registers', label: 'FC04 Read Input Registers' } ]
let disposed = false, activeKind: 'slave_scan' | 'register_scan' | null = null
let slaveTaskID: string | null = null, registerTaskID: string | null = null
let startingSlave = false, startingRegister = false, cancelRequested = false, stopAdding = false
const pendingSlaves = new Map<string, SlaveIdScanEvent>(), pendingRegisters = new Map<string, RegisterScanEvent>()
let unlistenSlave: (() => void) | undefined, unlistenReg: (() => void) | undefined
function close() { if (!busy.value) emit('close') }
function report(cause: unknown) { if (!disposed) error.value = String(cause) }
function finished() { scanning.value = false; cancelling.value = false; activeKind = null }
async function handleSlave(p: SlaveIdScanEvent) {
 if (disposed || p.ConnectionID !== connectionID) return
 if (startingSlave) { pendingSlaves.set(p.TaskID, p); return }
 if (p.TaskID !== slaveTaskID) return
 if (!Array.isArray(p.FoundIDs)) { report(t('scanDialog.invalidResponse')); slaveTaskID = null; finished(); return }
 slaveProgress.value = { CurrentID: p.CurrentID, Completed: p.Completed, Total: p.Total, FoundIDs: p.FoundIDs }
 if (p.Done || p.Cancelled) { slaveTaskID = null; finished(); slaveScanDone.value = true; slaveCancelled.value = p.Cancelled; if (p.Error) report(p.Error) }
}
async function handleRegister(p: RegisterScanEvent) {
 if (disposed || p.ConnectionID !== connectionID) return
 if (startingRegister) { pendingRegisters.set(p.TaskID, p); return }
 if (p.TaskID !== registerTaskID) return
 regProgress.value = { CurrentAddress: p.CurrentAddress, EndAddress: p.EndAddress, Completed: p.Completed, Total: p.Total, FoundRegisters: p.FoundRegisters }
 if (p.Done || p.Cancelled) {
  registerTaskID = null; finished(); regScanDone.value = true; registerCancelled.value = p.Cancelled
  if (p.Error) report(p.Error)
  else if (p.Done && !p.Cancelled && !cancelRequested && autoAddGroups.value && p.FoundRegisters.length) await addRegisterGroups()
 }
}
onMounted(async () => {
 unlistenSlave = EventsOn('scan-slave-progress', (p: SlaveIdScanEvent) => { void handleSlave(p).catch(report) })
 unlistenReg = EventsOn('scan-register-progress', (p: RegisterScanEvent) => { void handleRegister(p).catch(report) })
 try {
  const settings = await App.GetSettings(connectionID)
  if (disposed) return
  defaultSlaveID.value = settings.SlaveID
  regSlaveID.value = selectedGroup.value?.SlaveID ?? settings.SlaveID
 } catch (cause) { report(cause) }
 finally { if (!disposed) loading.value = false }
})
onUnmounted(() => {
 disposed = true; stopAdding = true; unlistenSlave?.(); unlistenReg?.()
 if (activeKind) void App.CancelScan(connectionID, activeKind).catch(() => {})
})
watch(selectedConnectionId, id => { if (id !== connectionID) emit('close') })
watch(maxChunk, maximum => { regChunkSize.value = Math.min(regChunkSize.value, maximum) })
const intIn = (v: number, min: number, max: number) => Number.isInteger(v) && v >= min && v <= max
async function startSlaveScan() {
 if (busy.value) return
 if (!intIn(slaveStartId.value, 1, 247) || !intIn(slaveEndId.value, slaveStartId.value, 247) || !intIn(slaveScanTimeout.value, 1, 600000)) { report(t('errors.invalidScanRange')); return }
 error.value = ''; scanning.value = true; cancelling.value = false; cancelRequested = false; startingSlave = true; activeKind = 'slave_scan'; slaveScanDone.value = false; slaveCancelled.value = false
 slaveProgress.value = { CurrentID: slaveStartId.value, Completed: 0, Total: slaveEndId.value - slaveStartId.value + 1, FoundIDs: [] }
 try {
  slaveTaskID = await App.StartSlaveIDScan(connectionID, { StartID: slaveStartId.value, EndID: slaveEndId.value, TimeoutMS: slaveScanTimeout.value })
  startingSlave = false
  if (disposed || cancelRequested) await App.CancelScan(connectionID, 'slave_scan')
  const pending = pendingSlaves.get(slaveTaskID); pendingSlaves.clear()
  if (pending) await handleSlave(pending)
 } catch (cause) { slaveTaskID = null; finished(); report(cause) }
 finally { startingSlave = false }
}
async function startRegisterScan() {
 if (busy.value) return
 if (!intIn(regSlaveID.value, 1, 247) || !intIn(regStartAddr.value, 0, 65535) || !intIn(regEndAddr.value, regStartAddr.value, 65535) || !intIn(regChunkSize.value, 1, maxChunk.value) || !intIn(regTimeout.value, 1, 600000)) { report(t('errors.invalidScanRange')); return }
 error.value = ''; addedMessage.value = ''; scanning.value = true; cancelling.value = false; cancelRequested = false; startingRegister = true; activeKind = 'register_scan'; regScanDone.value = false; registerCancelled.value = false; regPage.value = 0
 resultTarget.value = { slaveID: regSlaveID.value, fn: regFunction.value }
 regProgress.value = { CurrentAddress: regStartAddr.value, EndAddress: regEndAddr.value, Completed: 0, Total: regEndAddr.value - regStartAddr.value + 1, FoundRegisters: [] }
 try {
  registerTaskID = await App.StartRegisterScan(connectionID, { Function: regFunction.value, SlaveID: regSlaveID.value, StartAddress: regStartAddr.value, EndAddress: regEndAddr.value, ChunkSize: regChunkSize.value, TimeoutMS: regTimeout.value })
  startingRegister = false
  if (disposed || cancelRequested) await App.CancelScan(connectionID, 'register_scan')
  const pending = pendingRegisters.get(registerTaskID); pendingRegisters.clear()
  if (pending) await handleRegister(pending)
 } catch (cause) { registerTaskID = null; finished(); report(cause) }
 finally { startingRegister = false }
}
async function cancelScan() {
 if (adding.value) { stopAdding = true; return }
 if (!activeKind || cancelling.value) return
 cancelRequested = true; cancelling.value = true
 if (startingSlave || startingRegister) return
 try { await App.CancelScan(connectionID, activeKind) }
 catch (cause) { cancelling.value = false; report(cause) }
}
function slaveProgressPct() { return slaveProgress.value.Completed / slaveProgress.value.Total * 100 }
function regProgressPct() { return regProgress.value.Total ? regProgress.value.Completed / regProgress.value.Total * 100 : 0 }
function fmtHex(v: number) { return '0x' + (v & 65535).toString(16).toUpperCase().padStart(4, '0') }
function scanFoundSlave(id: number) { regSlaveID.value = id; tab.value = 'register'; error.value = '' }
async function addRegisterGroups() {
 const target = resultTarget.value
 if (!target || adding.value || !regProgress.value.FoundRegisters.length) return
 const addresses = [...new Set(regProgress.value.FoundRegisters.map(r => r.Address))].sort((a,b) => a-b)
 const ranges: { start: number; count: number }[] = []
 for (const address of addresses) { const last = ranges.at(-1); if (last && address === last.start + last.count && last.count < 65535) last.count++; else ranges.push({ start: address, count: 1 }) }
 adding.value = true; stopAdding = false; error.value = ''; let count = 0
 try {
  const existing = await App.ListScanGroups(connectionID)
  for (const range of ranges) {
   if (disposed || stopAdding) break
   if (existing.some(g => (g.SlaveID ?? defaultSlaveID.value) === target.slaveID && g.Function === target.fn && g.StartAddress === range.start && g.Quantity === range.count)) continue
   await App.AddScanGroup(connectionID, { ID: '', Name: `S${target.slaveID} ${target.fn} ${range.start}-${range.start+range.count-1}`, Function: target.fn, SlaveID: target.slaveID, StartAddress: range.start, Quantity: range.count, IntervalMS: 1000, Enabled: true, IsPolling: false })
   count++
  }
  if (!disposed) addedMessage.value = t('scanDialog.addedGroups', { count })
 } catch (cause) { report(cause) }
 finally { refreshTree(); adding.value = false }
}
</script>


<template>
  <Teleport to="body">
    <div class="modal-backdrop" @pointerdown.self.left="close">
      <div class="modal-box scan-dialog" v-modal="close" role="dialog" aria-modal="true" :aria-label="t('scanDialog.title')">
        <div class="modal-title">{{ t('scanDialog.title') }}</div>

        <p v-if="error" class="scan-error" role="alert">{{ error }}<br>{{ t('scanDialog.failureHint') }}</p>
        <div class="tab-bar">
          <button :class="['tab-btn', { active: tab === 'slave' }]" @click="tab = 'slave'" :disabled="busy">{{ t('scanDialog.slaveScan') }}</button>
          <button :class="['tab-btn', { active: tab === 'register' }]" @click="tab = 'register'" :disabled="busy">{{ t('scanDialog.registerScan') }}</button>
        </div>

        <!-- Slave ID Scan -->
        <div v-if="tab === 'slave'" class="tab-content">
          <div class="form-row-inline">
            <div class="form-row">
              <label for="scan-field-1">{{ t('scanDialog.startId') }}</label>
              <input id="scan-field-1" v-model.number="slaveStartId" type="number" min="1" max="247" :disabled="busy" />
            </div>
            <div class="form-row">
              <label for="scan-field-2">{{ t('scanDialog.endId') }}</label>
              <input id="scan-field-2" v-model.number="slaveEndId" type="number" min="1" max="247" :disabled="busy" />
            </div>
            <div class="form-row">
              <label for="scan-field-3">{{ t('dialog.timeout') }}</label>
              <input id="scan-field-3" v-model.number="slaveScanTimeout" type="number" min="100" max="600000" step="100" :disabled="busy" />
            </div>
          </div>

          <div class="scan-hint">{{ t('scanDialog.discoveryHint') }}</div>

          <div class="action-row">
            <button v-if="!scanning && !adding" :disabled="busy" class="btn-primary" @click="startSlaveScan">{{ t('scanDialog.startScan') }}</button>
            <button v-else-if="scanning" class="btn-danger" :disabled="cancelling" :aria-busy="cancelling" @click="cancelScan">{{ t(adding ? 'scanDialog.stopAdding' : 'common.cancel') }}</button>
          </div>

          <div v-if="scanning || slaveScanDone" class="progress-section">
            <div class="progress-bar-bg">
              <div class="progress-bar-fill" :style="{ width: slaveProgressPct() + '%' }"></div>
            </div>
            <div class="progress-text">{{ t('scanDialog.progressCount', { current: slaveProgress.Completed, total: slaveProgress.Total }) }} · ID {{ slaveProgress.CurrentID }}<span v-if="slaveCancelled"> · {{ t('scanDialog.cancelled') }}</span></div>
          </div>

          <div v-if="slaveProgress.FoundIDs.length > 0" class="result-section">
            <div class="result-title">{{ t('scanDialog.foundSlaves', { count: slaveProgress.FoundIDs.length }) }}</div>
            <div class="result-scroll">
              <table class="result-table">
                <thead><tr><th>{{ t('dialog.slaveId') }}</th><th>{{ t('scanDialog.nextStep') }}</th></tr></thead>
                <tbody>
                  <tr v-for="id in slaveProgress.FoundIDs" :key="id">
                    <td>{{ id }}</td><td><button class="btn-secondary" :disabled="busy" @click="scanFoundSlave(id)">{{ t('scanDialog.scanThisSlave') }}</button></td>
                  </tr>
                </tbody>
              </table>
            </div>
          </div>
          <div v-else-if="slaveScanDone && !error" class="empty-result">{{ t('scanDialog.noSlavesFound') }}</div>
        </div>

        <!-- Register Scan -->
        <div v-if="tab === 'register'" class="tab-content">
          <div class="form-row"><label for="scan-slave-id">{{ t('dialog.slaveId') }}</label><input id="scan-slave-id" v-model.number="regSlaveID" type="number" min="1" max="247" :disabled="busy" /></div>
          <div class="form-row">
            <label for="scan-field-4">{{ t('table.function') }}</label>
              <select id="scan-field-4" v-model="regFunction" :disabled="busy">
              <option v-for="opt in functionOptions" :key="opt.value" :value="opt.value">{{ opt.label }}</option>
            </select>
          </div>
          <div class="form-row-inline">
            <div class="form-row">
              <label for="scan-field-5">{{ t('table.startAddress') }}</label>
              <input id="scan-field-5" v-model.number="regStartAddr" type="number" min="0" max="65535" :disabled="busy" />
            </div>
            <div class="form-row">
              <label for="scan-field-6">{{ t('table.endAddress') }}</label>
              <input id="scan-field-6" v-model.number="regEndAddr" type="number" min="0" max="65535" :disabled="busy" />
            </div>
          </div>
          <div class="form-row-inline">
            <div class="form-row">
              <label for="scan-field-7">{{ t('scanDialog.chunkSize') }}</label>
              <input id="scan-field-7" v-model.number="regChunkSize" type="number" min="1" :max="maxChunk" :disabled="busy" />
            </div>
            <div class="form-row">
              <label for="scan-field-8">{{ t('dialog.timeout') }}</label>
              <input id="scan-field-8" v-model.number="regTimeout" type="number" min="100" max="600000" step="100" :disabled="busy" />
            </div>
          </div>

          <label class="checkbox-row">
            <input type="checkbox" v-model="autoAddGroups" :disabled="busy" />
            <span>{{ t('scanDialog.autoAddAfterScan') }}</span>
          </label>

          <div class="action-row">
            <button v-if="!scanning && !adding" :disabled="busy" class="btn-primary" @click="startRegisterScan">{{ t('scanDialog.startScan') }}</button>
            <button v-else class="btn-danger" :disabled="cancelling" :aria-busy="cancelling" @click="cancelScan">{{ t(adding ? 'scanDialog.stopAdding' : 'common.cancel') }}</button>
          </div>

          <div v-if="scanning || regScanDone" class="progress-section">
            <div class="progress-bar-bg">
              <div class="progress-bar-fill" :style="{ width: regProgressPct() + '%' }"></div>
            </div>
            <div class="progress-text">{{ t('scanDialog.progressCount', { current: regProgress.Completed, total: regProgress.Total }) }}<span v-if="registerCancelled"> · {{ t('scanDialog.cancelled') }}</span></div>
          </div>

          <div v-if="regProgress.FoundRegisters.length > 0" class="result-section">
            <div class="result-title">{{ t('scanDialog.foundRegisters', { count: regProgress.FoundRegisters.length }) }} · ID {{ resultTarget?.slaveID }}</div>
            <div class="result-scroll">
              <table class="result-table">
                <thead><tr><th>{{ t('table.address') }}</th><th>Hex</th><th>{{ t('dialog.simpleValue') }}</th></tr></thead>
                <tbody>
                  <tr v-for="reg in visibleRegisters" :key="reg.Address">
                    <td>{{ reg.Address }}</td>
                    <td class="mono">{{ fmtHex(reg.Value) }}</td>
                    <td class="mono">{{ reg.Value }}</td>
                  </tr>
                </tbody>
              </table>
            </div>
          </div>
          <div v-if="regPages > 1" class="result-pages"><button class="btn-secondary" :disabled="regPage === 0" @click="regPage--">{{ t('scanDialog.previousPage') }}</button><span>{{ regPage + 1 }} / {{ regPages }}</span><button class="btn-secondary" :disabled="regPage + 1 >= regPages" @click="regPage++">{{ t('scanDialog.nextPage') }}</button></div>
          <button v-if="regScanDone && regProgress.FoundRegisters.length" class="btn-primary add-results" :disabled="busy" @click="addRegisterGroups">{{ t('scanDialog.addResults') }}</button>
          <p v-if="addedMessage" role="status" class="scan-hint">{{ addedMessage }}</p>
          <div v-if="regScanDone && !regProgress.FoundRegisters.length && !error" class="empty-result">{{ t('scanDialog.noRegistersFound') }}</div>
        </div>

        <div class="modal-footer">
          <button class="btn-secondary" @click="close" :disabled="busy">{{ t('common.close') }}</button>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.modal-backdrop { position: fixed; top: 0; left: 0; right: 0; bottom: 0; background: rgba(0,0,0,0.6); display: flex; align-items: center; justify-content: center; z-index: 1000; }
.modal-box { background: var(--c-base); border: 1px solid var(--c-surface1); border-radius: 8px; width: 520px; max-width: calc(100vw - 32px); max-height: 80vh; display: flex; flex-direction: column; }
.modal-title { padding: 12px 16px; font-size: 14px; font-weight: 600; color: var(--c-text); border-bottom: 1px solid var(--c-surface0); }
.modal-footer { padding: 8px 16px; border-top: 1px solid var(--c-surface0); display: flex; justify-content: flex-end; }

.tab-bar { display: flex; border-bottom: 1px solid var(--c-surface0); }
.tab-btn { flex: 1; padding: 8px; background: transparent; border: none; color: var(--c-subtext0); cursor: pointer; font-size: 12px; border-bottom: 2px solid transparent; }
.tab-btn.active { color: var(--c-blue); border-bottom-color: var(--c-blue); }
.tab-btn:disabled { opacity: 0.5; cursor: not-allowed; }

.tab-content { padding: 12px 16px; overflow-y: auto; flex: 1; min-height: 0; }

.form-row { margin-bottom: 8px; }
.form-row label { display: block; font-size: 11px; color: var(--c-subtext0); margin-bottom: 3px; }
.form-row input, .form-row select { width: 100%; padding: 5px 8px; background: var(--c-control-bg); border: 1px solid var(--c-surface1); border-radius: 4px; color: var(--c-text); font-size: 12px; }
.form-row input:focus, .form-row select:focus { outline: none; border-color: var(--c-blue); }
.form-row-inline { display: flex; gap: 8px; }
.form-row-inline .form-row { flex: 1; min-width: 0; }

.checkbox-row { display: flex; align-items: center; gap: 6px; font-size: 12px; color: var(--c-text); cursor: pointer; margin-bottom: 8px; }
.checkbox-row input { accent-color: var(--c-blue); }
.action-row { margin: 10px 0; }
.btn-primary { width: 100%; padding: 6px; background: var(--c-blue); color: var(--c-on-accent); border: none; border-radius: 4px; cursor: pointer; font-size: 12px; font-weight: 600; }
.btn-primary:hover { background: var(--c-accent-hover); }
.btn-danger { width: 100%; padding: 6px; background: var(--c-red); color: var(--c-on-accent); border: none; border-radius: 4px; cursor: pointer; font-size: 12px; font-weight: 600; }
.btn-secondary { padding: 5px 16px; background: var(--c-control-bg); color: var(--c-text); border: 1px solid var(--c-surface1); border-radius: 4px; cursor: pointer; font-size: 12px; }
.btn-secondary:disabled { opacity: 0.5; cursor: not-allowed; }

.progress-section { margin: 8px 0; }
.progress-bar-bg { height: 6px; background: var(--c-control-bg); border-radius: 3px; overflow: hidden; }
.progress-bar-fill { height: 100%; background: var(--c-blue); transition: width 0.15s ease; }
.progress-text { font-size: 11px; color: var(--c-subtext0); text-align: center; margin-top: 4px; }

.result-section { margin-top: 8px; }
.result-title { font-size: 12px; color: var(--c-green); margin-bottom: 4px; font-weight: 600; }
.result-scroll { max-height: 200px; overflow-y: auto; border: 1px solid var(--c-surface0); border-radius: 4px; }
.result-table { width: 100%; border-collapse: collapse; font-size: 12px; }
.result-table th { background: var(--c-mantle); color: var(--c-subtext0); font-weight: 500; padding: 4px 10px; text-align: left; position: sticky; top: 0; }
.result-table td { padding: 3px 10px; border-top: 1px solid var(--c-base); color: var(--c-text); }
.result-table .mono { font-family: var(--font-mono); }
.empty-result { text-align: center; color: var(--c-subtext0); font-size: 12px; padding: 16px; }
.scan-hint { font-size: 11px; color: var(--c-subtext0); margin-bottom: 8px; }
.scan-error { color: var(--c-red); padding: 10px 16px; font-size: 12px; overflow-wrap: anywhere; }
.result-pages { display: flex; align-items: center; justify-content: space-between; gap: 8px; margin: 8px 0; }
.add-results { margin-top: 10px; }
button:disabled { opacity: .5; cursor: not-allowed; }
.status-text { font-size: 12px; color: var(--c-blue); text-align: center; padding: 6px; }
</style>
