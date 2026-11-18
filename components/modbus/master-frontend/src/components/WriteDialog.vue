<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import * as App from '../../wailsjs/go/main/App'
import { useI18n, showAlert, vModal, encodeRegisterWrite, registerWriteOrders, registerWriteWidth, type RegisterWriteType } from 'shared-frontend'

const { t } = useI18n()

interface Props { show: boolean; connectionId: string | null; slaveId?: number | null }
const props = defineProps<Props>()
const writeTarget = computed(() => props.slaveId == null ? t('valuePanel.writeDefaultTarget') : t('valuePanel.writeTarget', { id: props.slaveId }))
const emit = defineEmits<{ (e: 'close'): void }>()

const form = ref({
  Function: 'write_single_register',
  Address: 0,
  value: '',
  dataType: 'raw' as RegisterWriteType,
  byteOrder: 'AB',
})

watch(() => props.show, (visible) => {
  if (!visible) return
  form.value = { Function: 'write_single_register', Address: 0, value: '', dataType: 'raw', byteOrder: 'AB' }
})

watch(() => form.value.Function, (fc) => {
  form.value.dataType = 'raw'
  if (fc === 'write_multiple_coils' && !form.value.value.trim()) form.value.value = '0'
})

const busy = ref(false)
function close() { if (!busy.value) emit('close') }
const isMultiCoilFC = computed(() => form.value.Function === 'write_multiple_coils')
// Keep empty comma-separated entries so an omitted value cannot shift later addresses.
const coilTokens = computed(() => {
  const input = form.value.value.trim()
  return input ? input.split(/[,，]/).flatMap(part => part.trim() ? part.trim().split(/\s+/) : ['']) : []
})
const coilValues = computed(() => coilTokens.value.map(value => /^(1|true|on)$/i.test(value)))
const invalidQuantity = ref(false)
watch(() => form.value.value, () => { invalidQuantity.value = false })
const coilQuantity = computed({
  get: () => coilTokens.value.length,
  set: (Quantity: number | string) => {
    const count = Number(Quantity)
    invalidQuantity.value = !Number.isInteger(count) || count < 1 || count > 1968
    if (invalidQuantity.value) return
    form.value.value = Array.from({ length: count }, (_, i) => coilTokens.value[i] ?? '0').join(', ')
  },
})
const coilError = computed(() => {
  if (!isMultiCoilFC.value) return ''
  if (invalidQuantity.value || coilTokens.value.length < 1 || coilTokens.value.length > 1968) return t('errors.invalidCoilQuantity')
  const invalidIndex = coilTokens.value.findIndex(value => !/^(0|1|true|false|on|off)$/i.test(value))
  if (invalidIndex >= 0) return t('errors.invalidCoilValue', { index: invalidIndex + 1 })
  if (!Number.isInteger(form.value.Address) || form.value.Address < 0 || form.value.Address > 65535) return t('errors.invalidCoilAddress')
  if (form.value.Address + coilTokens.value.length - 1 > 65535) return t('errors.addressOverflow')
  return ''
})

function toggleCoil(index: number) {
  const tokens = [...coilTokens.value]
  tokens[index] = coilValues.value[index] ? '0' : '1'
  form.value.value = tokens.join(', ')
}

const isMultiRegFC = computed(() => form.value.Function === 'write_multiple_registers')
const registerTypes: RegisterWriteType[] = ['raw', 'uint16', 'int16', 'uint32', 'int32', 'uint64', 'int64', 'float32', 'float64']
const byteOrders = computed(() => registerWriteOrders(form.value.dataType))
watch(() => form.value.dataType, () => {
  if (!byteOrders.value.includes(form.value.byteOrder)) form.value.byteOrder = byteOrders.value[0]!
})
const registerTokens = computed(() => {
  const input = form.value.value.trim()
  return input ? input.split(/[,，]/).flatMap(part => part.trim() ? part.trim().split(/\s+/) : ['']) : []
})
const registerPreview = computed(() => {
  if (!isMultiRegFC.value) return []
  return registerTokens.value.map((input, index) => {
    try {
      return { input, index, regs: encodeRegisterWrite(input, form.value.dataType, form.value.byteOrder), error: '' }
    } catch {
      return { input, index, regs: [] as number[], error: t('errors.invalidRegisterValue', { index: index + 1, type: form.value.dataType }) }
    }
  })
})
const registerCount = computed(() => registerTokens.value.length * registerWriteWidth(form.value.dataType))
const registerAddressError = computed(() => {
  if (!Number.isInteger(form.value.Address) || form.value.Address < 0 || form.value.Address > 65535) return t('errors.invalidRegisterAddress')
  if (form.value.Address + registerCount.value - 1 > 65535) return t('errors.addressOverflow')
  return ''
})
const registerValueError = computed(() => {
  if (!registerTokens.value.length) return t('errors.emptyRegisterValues')
  if (registerCount.value > 123) return t('errors.overflowFC16', { count: registerCount.value })
  return registerPreview.value.find(row => row.error)?.error ?? ''
})
const registerError = computed(() => isMultiRegFC.value ? registerAddressError.value || registerValueError.value : '')
const registerPlaceholder = computed(() => {
  if (form.value.dataType.startsWith('float')) return '3.14, 2.71'
  if (form.value.dataType === 'int64') return '-9223372036854775808, 9223372036854775807'
  if (form.value.dataType === 'uint64') return '18446744073709551615'
  return form.value.dataType.startsWith('int') ? '-1, 123' : '10, 20'
})

async function submit() {
  if (!props.connectionId || busy.value || (isMultiCoilFC.value && coilError.value) || registerError.value) return
  busy.value = true
  try {
    const fc = form.value.Function
    if (fc === 'write_single_register') {
      await App.WriteSingleRegisterSlave(props.connectionId, props.slaveId ?? null, form.value.Address, parseInt(form.value.value))
    } else if (fc === 'write_single_coil') {
      await App.WriteSingleCoilSlave(props.connectionId, props.slaveId ?? null, form.value.Address, form.value.value === '1' || form.value.value.toLowerCase() === 'true')
    } else if (fc === 'write_multiple_registers') {
      const values = registerPreview.value.flatMap(row => row.regs)
      await App.WriteMultipleRegistersSlave(props.connectionId, props.slaveId ?? null, form.value.Address, values)
    } else if (fc === 'write_multiple_coils') {
      const values = [...coilValues.value]
      await App.WriteMultipleCoilsSlave(props.connectionId, props.slaveId ?? null, form.value.Address, values)
    }
    emit('close')
  } catch (e) { await showAlert(String(e)) }
  finally { busy.value = false }
}
</script>

<template>
  <Teleport to="body">
    <div v-if="show" class="modal-backdrop" @pointerdown.self.left="close">
      <div v-modal="close" :aria-label="t('dialog.writeRegisters')" :aria-busy="busy" class="modal-box" :class="{ 'coil-modal': isMultiCoilFC, 'register-modal': isMultiRegFC }">
        <div class="modal-title">{{ t('dialog.writeRegisters') }}</div>
        <div class="modal-scroll">
        <fieldset class="modal-body" :disabled="busy">
          <div class="form-hint write-target">{{ writeTarget }}</div>
          <label class="form-label">
            {{ t('table.function') }}
            <select v-model="form.Function" class="form-input">
              <option value="write_single_coil">FC05 - Write Single Coil</option>
              <option value="write_single_register">FC06 - Write Single Register</option>
              <option value="write_multiple_coils">FC15 - Write Multiple Coils</option>
              <option value="write_multiple_registers">FC16 - Write Multiple Registers</option>
            </select>
          </label>
          <label v-if="isMultiRegFC" class="form-label">
            {{ t('dialog.dataType') }}
            <select v-model="form.dataType" class="form-input">
              <option v-for="type in registerTypes" :key="type" :value="type">{{ t(`dialog.writeType_${type}`) }}</option>
            </select>
          </label>
          <label v-if="isMultiRegFC" class="form-label">
            {{ t('dialog.byteOrder') }}
            <select v-model="form.byteOrder" class="form-input">
              <option v-for="order in byteOrders" :key="order" :value="order">{{ order }}</option>
            </select>
          </label>
          <label class="form-label">
            {{ isMultiCoilFC ? t('dialog.coilStartAddress') : t('table.address') }}
            <input v-model.number="form.Address" class="form-input" type="number" min="0" max="65535" :aria-invalid="isMultiRegFC && !!registerAddressError" :aria-describedby="isMultiRegFC && registerAddressError ? 'register-write-error' : undefined" />
          </label>
          <label v-if="isMultiCoilFC" class="form-label">
            {{ t('dialog.coilQuantity') }}
            <input v-model="coilQuantity" class="form-input" type="number" min="1" max="1968" step="1" :disabled="busy"
              :aria-invalid="invalidQuantity" :aria-describedby="invalidQuantity ? 'coil-value-error' : undefined" />
          </label>
          <label class="form-label">
            {{ t('dialog.simpleValue') }}
            <span id="coil-value-hint" class="form-hint" v-if="isMultiCoilFC">{{ t('dialog.coilValuesHint') }}</span>
            <span id="register-value-hint" class="form-hint" v-else-if="isMultiRegFC">{{ t(form.dataType.startsWith('float') ? 'dialog.registerFloatHint' : 'dialog.registerIntegerHint') }}</span>
            <span class="form-hint" v-else-if="form.Function.includes('multiple')">{{ t('dialog.valueHintMultiple') }}</span>
            <textarea v-if="isMultiCoilFC" v-model="form.value" class="form-input form-textarea" placeholder="1, 0, 1, 0" rows="3"
              :disabled="busy" :aria-invalid="!!coilError" :aria-describedby="coilError ? 'coil-value-hint coil-value-error' : 'coil-value-hint'" />
            <textarea v-else-if="isMultiRegFC" v-model="form.value" class="form-input form-textarea" :placeholder="registerPlaceholder" rows="3"
              :aria-invalid="!!registerValueError" :aria-describedby="registerError ? 'register-value-hint register-write-error' : 'register-value-hint'" />
            <input v-else v-model="form.value" class="form-input" type="text" placeholder="0" />
          </label>
          <template v-if="isMultiCoilFC">
            <p v-if="coilError" id="coil-value-error" class="coil-error" role="alert">{{ coilError }}</p>
            <template v-else>
              <p class="coil-summary" role="status">{{ t('dialog.coilSummary', { count: coilQuantity, start: form.Address, end: form.Address + coilQuantity - 1 }) }}</p>
              <div class="coil-grid">
                <button v-for="(value, index) in coilValues" :key="index" class="coil-toggle" :class="{ on: value }"
                  :aria-label="t('dialog.coilAtAddress', { Address: form.Address + index })" :aria-pressed="value" :disabled="busy" @click="toggleCoil(index)">
                  <span>{{ form.Address + index }}</span><strong>{{ value ? 'ON' : 'OFF' }}</strong>
                </button>
              </div>
            </template>
          </template>
          <p v-if="registerError" id="register-write-error" class="coil-error" role="alert">{{ registerError }}</p>
          <div v-if="isMultiRegFC && registerPreview.length && !registerError" class="register-preview">
            <p class="preview-summary" role="status">{{ t('dialog.registerWriteSummary', { count: registerPreview.length, regCount: registerCount, start: form.Address, end: form.Address + registerCount - 1 }) }}</p>
            <table class="preview-table">
              <thead><tr><th>{{ t('table.address') }}</th><th>{{ t('dialog.simpleValue') }}</th><th>{{ t('dialog.registerWriteWords') }}</th></tr></thead>
              <tbody>
                <tr v-for="item in registerPreview" :key="item.index">
                  <td>{{ form.Address + item.index * registerWriteWidth(form.dataType) }}</td>
                  <td>{{ item.input }}</td>
                  <td>{{ item.regs.map(reg => '0x' + reg.toString(16).toUpperCase().padStart(4, '0')).join(' ') }}</td>
                </tr>
              </tbody>
            </table>
          </div>
        </fieldset>
        </div>
        <div class="modal-footer">
          <button class="btn btn-secondary" :disabled="busy" @click="close">{{ t('common.cancel') }}</button>
          <button class="btn btn-primary" @click="submit"
            :aria-busy="busy" :disabled="busy || (isMultiCoilFC && !!coilError) || !!registerError">{{ t('toolbar.write') }}</button>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.modal-scroll { min-height: 0; overflow-y: auto; overscroll-behavior: contain; padding-right: 8px; }
.modal-backdrop { position: fixed; inset: 0; background: rgba(0,0,0,0.5); display: flex; align-items: center; justify-content: center; z-index: 1000; }
.modal-box { background: var(--c-base); border: 1px solid var(--c-surface1); border-radius: 8px; padding: 20px; box-sizing: border-box; width: 400px; max-width: calc(100vw - 32px); max-height: calc(100vh - 32px); overflow: hidden; display: flex; flex-direction: column; box-shadow: 0 8px 24px rgba(0,0,0,0.5); }
.coil-modal { width: min(520px, calc(100vw - 32px)); min-width: 0; box-sizing: border-box; max-height: calc(100vh - 32px); overflow-y: auto; }
.register-modal { width: min(560px, calc(100vw - 32px)); }
.coil-modal .form-label, .coil-modal .form-hint { color: var(--c-subtext1); }
.coil-error { margin: 0; color: var(--c-red); font-size: 12px; }
.coil-summary { margin: 0; color: var(--c-green); font-size: 12px; }
.coil-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(84px, 1fr)); gap: 8px; max-height: 200px; overflow-y: auto; padding: 3px; }
.coil-toggle { display: flex; align-items: center; justify-content: space-between; gap: 8px; padding: 8px; border: 1px solid var(--c-surface2); border-radius: 4px; background: var(--c-control-bg); color: var(--c-text); font-family: monospace; cursor: pointer; }
.coil-toggle.on { background: var(--c-success-bg); color: var(--c-success-text); border-color: var(--c-green); }
.coil-toggle:focus-visible { outline: 2px solid var(--c-blue); outline-offset: 2px; }
.coil-toggle:disabled { cursor: default; opacity: 0.6; }
.modal-title { flex-shrink: 0;  font-size: 15px; font-weight: 600; color: var(--c-text); margin-bottom: 16px; }
.modal-body {  border: 0; padding: 0; margin: 0; min-width: 0; display: flex; flex-direction: column; gap: 12px; }
.modal-footer { flex-shrink: 0;  display: flex; justify-content: flex-end; gap: 8px; margin-top: 20px; }
.form-label { display: flex; flex-direction: column; gap: 4px; font-size: 12px; color: var(--c-subtext0); }
.form-input { min-width: 0; padding: 6px 10px; background: var(--c-control-bg); border: 1px solid var(--c-surface1); border-radius: 4px; color: var(--c-text); font-size: 13px; }
.form-input:focus { outline: none; border-color: var(--c-blue); }
.form-hint { color: var(--c-subtext0); font-size: 11px; }
.form-textarea { resize: vertical; min-height: 60px; font-family: var(--font-mono); }
.btn { padding: 7px 20px; border: none; border-radius: 6px; cursor: pointer; font-size: 13px; }
.btn-primary { background: var(--c-blue); color: var(--c-on-accent); }
.btn-primary:hover { background: var(--c-accent-hover); }
.btn-primary:disabled { opacity: 0.4; cursor: default; }
.btn-secondary { background: var(--c-neutral-bg); color: var(--c-text); }
.btn-secondary:hover { background: var(--c-hover-bg); }
.register-preview { margin-top: 4px; border: 1px solid var(--c-surface0); border-radius: 4px; overflow-x: auto; }
.preview-summary { padding: 4px 8px; font-size: 11px; color: var(--c-green); background: var(--c-mantle); }
.preview-table { width: 100%; border-collapse: collapse; font-size: 11px; }
.preview-table th { background: var(--c-mantle); color: var(--c-subtext0); font-weight: 500; padding: 3px 8px; text-align: left; }
.preview-table td { padding: 2px 8px; color: var(--c-text); font-family: var(--font-mono); border-top: 1px solid var(--c-base); }
</style>
