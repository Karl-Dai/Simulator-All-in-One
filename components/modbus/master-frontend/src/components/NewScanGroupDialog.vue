<script setup lang="ts">
import { ref, watch } from 'vue'
import * as App from '../../wailsjs/go/main/App'
import { useI18n, showAlert, vModal } from 'shared-frontend'
import type { ScanGroupInfo } from '../types'

const { t } = useI18n()

interface Props { show: boolean; connectionId: string | null; group?: ScanGroupInfo }
const props = defineProps<Props>()
const emit = defineEmits<{ (e: 'close'): void; (e: 'created'): void; (e: 'saved', group: ScanGroupInfo): void }>()
const busy = ref(false)
const useDefaultSlave = ref(true)
const slaveID = ref(1)
function close() { if (!busy.value) emit('close') }

const form = ref({
  Name: '',
  Function: 'read_holding_registers',
  StartAddress: 0,
  Quantity: 10,
  IntervalMS: 1000,
})

watch(() => [props.show, props.group] as const, ([visible, group]) => {
  if (!visible) return
  useDefaultSlave.value = group?.SlaveID == null
  slaveID.value = group?.SlaveID ?? 1
  form.value = group ? { Name: group.Name, Function: group.Function, StartAddress: group.StartAddress, Quantity: group.Quantity, IntervalMS: group.IntervalMS }
    : { Name: '', Function: 'read_holding_registers', StartAddress: 0, Quantity: 10, IntervalMS: 1000 }
}, { immediate: true })

async function submit() {
  if (!props.connectionId || busy.value) return
  if (!useDefaultSlave.value && (!Number.isInteger(slaveID.value) || slaveID.value < 1 || slaveID.value > 247)) {
    await showAlert(t('errors.invalidSlaveId'))
    return
  }
  if (!Number.isInteger(form.value.StartAddress) || form.value.StartAddress < 0
    || !Number.isInteger(form.value.Quantity) || form.value.Quantity < 1 || form.value.Quantity > 65535
    || form.value.StartAddress + form.value.Quantity > 65536
    || !Number.isInteger(form.value.IntervalMS) || form.value.IntervalMS < 100 || form.value.IntervalMS > 60000) {
    await showAlert(t('errors.invalidScanRange'))
    return
  }
  busy.value = true
  try {
    const group: ScanGroupInfo = {
      ...(props.group ?? { ID: '', Enabled: true, IsPolling: false }),
      ...form.value, SlaveID: useDefaultSlave.value ? undefined : slaveID.value, Name: form.value.Name || `SG-${Date.now() % 10000}`,
    }
    if (props.group) {
      await App.UpdateScanGroup(props.connectionId, group)
      emit('saved', group)
    } else {
      await App.AddScanGroup(props.connectionId, group)
      emit('created')
    }
    emit('close')
  } catch (e) { await showAlert(String(e)) }
  finally { busy.value = false }
}
</script>

<template>
  <Teleport to="body">
    <div v-if="show" class="modal-backdrop" @pointerdown.self.left="close">
      <div class="modal-box" v-modal="close" :aria-label="t(group ? 'dialog.editScanGroup' : 'dialog.newScanGroup')">
        <div class="modal-title">{{ t(group ? 'dialog.editScanGroup' : 'dialog.newScanGroup') }}</div>
        <p v-if="group" class="form-hint">{{ t('dialog.editScanGroupHint') }}</p>
        <form class="modal-form" @submit.prevent="submit">
        <div class="modal-scroll">
        <fieldset class="modal-body" :disabled="busy">
          <label class="form-label">
            {{ t('dialog.simpleName') }}
            <input v-model="form.Name" class="form-input" type="text" :placeholder="t('dialog.scanGroupName')" />
          </label>
          <label class="form-label">
            {{ t('dialog.pollSlaveTarget') }}
            <select id="poll-slave-mode" v-model="useDefaultSlave" class="form-input">
              <option :value="true">{{ t('dialog.followDefaultSlave') }}</option>
              <option :value="false">{{ t('dialog.specifySlave') }}</option>
            </select>
          </label>
          <label v-if="!useDefaultSlave" class="form-label">
            {{ t('dialog.slaveId') }}
            <input id="poll-slave-id" v-model.number="slaveID" class="form-input" type="number" min="1" max="247" />
          </label>
          <p class="form-hint">{{ t('dialog.multiSlaveHint') }}</p>
          <label class="form-label">
            {{ t('table.function') }}
            <select v-model="form.Function" class="form-input">
              <option value="read_coils">FC01 - Read Coils</option>
              <option value="read_discrete_inputs">FC02 - Read Discrete Inputs</option>
              <option value="read_holding_registers">FC03 - Read Holding Registers</option>
              <option value="read_input_registers">FC04 - Read Input Registers</option>
            </select>
          </label>
          <label class="form-label">
            {{ t('table.startAddress') }}
            <input v-model.number="form.StartAddress" class="form-input" type="number" min="0" max="65535" />
          </label>
          <label class="form-label">
            {{ t('table.quantity') }}
            <input id="poll-quantity" v-model.number="form.Quantity" class="form-input" type="number" min="1" :max="Math.min(65535, 65536 - form.StartAddress)" aria-describedby="scan-Quantity-hint" />
          </label>
          <p id="scan-Quantity-hint" class="form-hint">{{ t('dialog.scanQuantityHint') }}</p>
          <label class="form-label">
            {{ t('dialog.scanInterval') }}
            <input v-model.number="form.IntervalMS" class="form-input" type="number" min="100" max="60000" aria-describedby="scan-interval-hint" />
          </label>
          <p id="scan-interval-hint" class="form-hint">{{ t('dialog.scanIntervalHint') }}</p>
        </fieldset>
        </div>
        <div class="modal-footer">
          <button type="button" class="btn btn-secondary"  :disabled="busy" @click="close">{{ t('common.cancel') }}</button>
          <button type="submit" class="btn btn-primary" :disabled="busy">{{ t(busy ? 'common.loading' : group ? 'common.save' : 'common.create') }}</button>
        </div>
        </form>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.modal-scroll { min-height: 0; overflow-y: auto; overscroll-behavior: contain; padding-right: 8px; }
.modal-form { display: flex; flex-direction: column; min-height: 0; overflow: hidden; }

.modal-backdrop { position: fixed; inset: 0; background: rgba(0,0,0,0.5); display: flex; align-items: center; justify-content: center; z-index: 1000; }
.modal-box { box-sizing: border-box; background: var(--c-base); border: 1px solid var(--c-surface1); border-radius: 8px; padding: 20px; width: 400px; max-width: calc(100vw - 32px); max-height: calc(100vh - 32px); overflow: hidden; display: flex; flex-direction: column; box-shadow: 0 8px 24px rgba(0,0,0,0.5); }
.modal-title { flex-shrink: 0;  font-size: 15px; font-weight: 600; color: var(--c-text); margin-bottom: 16px; }
.modal-body {  border: 0; padding: 0; margin: 0; min-width: 0; display: flex; flex-direction: column; gap: 12px; }
.modal-footer { flex-shrink: 0;  display: flex; justify-content: flex-end; gap: 8px; margin-top: 20px; }
.form-label { display: flex; flex-direction: column; gap: 4px; font-size: 12px; color: var(--c-subtext0); }
.form-hint { margin: -6px 0 0; color: var(--c-subtext0); font-size: 12px; line-height: 1.5; }
.form-input { padding: 6px 10px; background: var(--c-control-bg); border: 1px solid var(--c-surface1); border-radius: 4px; color: var(--c-text); font-size: 13px; }
.form-input:focus { outline: none; border-color: var(--c-blue); }
.btn { padding: 7px 20px; border: none; border-radius: 6px; cursor: pointer; font-size: 13px; }
.btn-primary { background: var(--c-blue); color: var(--c-on-accent); }
.btn-primary:hover { background: var(--c-accent-hover); }
.btn-secondary { background: var(--c-neutral-bg); color: var(--c-text); }
.btn-secondary:hover { background: var(--c-hover-bg); }
</style>
