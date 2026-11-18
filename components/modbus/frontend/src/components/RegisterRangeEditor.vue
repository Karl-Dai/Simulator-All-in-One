<script setup lang="ts">
import { computed, useId } from 'vue'
import { useI18n } from 'shared-frontend'
import { REGISTER_AREAS, rangeError, rangeCounts, type RegisterRange } from '../utils/registerRanges'

const ranges = defineModel<RegisterRange[]>({ required: true })
defineProps<{ disabled?: boolean }>()
const { t } = useI18n()
const editorId = useId()
const rows = computed(() => ranges.value.map((range, index) => ({ range, index })))
const summary = computed(() => ranges.value.reduce((sum, range, index) => {
  if (rangeError(ranges.value, index)) return sum
  const counts = rangeCounts(range)
  return { points: sum.points + counts.points, addresses: sum.addresses + counts.addresses }
}, { points: 0, addresses: 0 }))
function add(type: string, bit: boolean) {
  const end = Math.max(-1, ...ranges.value.filter(r => r.RegisterType === type).map(r => r.EndAddress))
  const start = Math.min(end + 1, 65535)
  ranges.value = [...ranges.value, { RegisterType: type, StartAddress: start, EndAddress: start, DataType: bit ? 'bool' : 'uint16', Endian: 'big' }]
}
function remove(index: number) {
  ranges.value = ranges.value.filter((_, i) => i !== index)
}
</script>

<template>
  <section class="range-editor" :aria-label="t('ranges.title')">
    <h3>{{ t('ranges.title') }}</h3>
    <p class="range-hint">{{ t('ranges.help') }}</p>
    <fieldset v-for="area in REGISTER_AREAS" :key="area.type" :disabled="disabled" class="range-area" :data-area="area.type">
      <legend>{{ t(area.label) }} <span>{{ area.fc }}</span></legend>
      <div v-for="{ range, index } in rows.filter(row => row.range.RegisterType === area.type)" :key="index" class="range-row" :data-range-index="index">
        <div class="range-fields">
          <label>{{ t('table.startAddress') }}
            <input v-model.number="range.StartAddress" type="number" min="0" max="65535" step="1" :aria-invalid="!!rangeError(ranges, index)" :aria-describedby="`${editorId}-${index}-error`" />
          </label>
          <label>{{ t('table.endAddress') }}
            <input v-model.number="range.EndAddress" type="number" min="0" max="65535" step="1" :aria-invalid="!!rangeError(ranges, index)" :aria-describedby="`${editorId}-${index}-error`" />
          </label>
          <label>{{ t('dialog.dataType') }}
            <select v-model="range.DataType" :aria-describedby="`${editorId}-${index}-error`">
              <option v-if="area.bit" value="bool">Bool</option>
              <template v-else>
                <option value="uint16">UInt16</option><option value="int16">Int16</option>
                <option value="uint32">UInt32</option><option value="int32">Int32</option><option value="float32">Float32</option>
              </template>
            </select>
          </label>
          <label v-if="!area.bit">{{ t('dialog.byteOrder') }}
            <select v-model="range.Endian">
              <option value="big">{{ t('dialog.byteOrderBig') }}</option><option value="little">{{ t('dialog.byteOrderLittle') }}</option>
              <option value="mid_big">{{ t('dialog.byteOrderMidBig') }}</option><option value="mid_little">{{ t('dialog.byteOrderMidLittle') }}</option>
            </select>
          </label>
          <button type="button" class="range-remove" @click="remove(index)">{{ t('ranges.remove') }}</button>
        </div>
        <p v-if="rangeError(ranges, index)" :id="`${editorId}-${index}-error`" class="range-error" role="alert">{{ t(rangeError(ranges, index)!) }}</p>
        <p v-else class="range-count">{{ t('ranges.count', rangeCounts(range)) }}</p>
      </div>
      <p v-if="!rows.some(row => row.range.RegisterType === area.type)" class="range-empty">{{ t('ranges.empty') }}</p>
      <button type="button" class="range-add" @click="add(area.type, area.bit)">{{ t('ranges.add') }}</button>
    </fieldset>
    <p class="range-total" aria-live="polite">{{ t('ranges.total', summary) }}</p>
  </section>
</template>

<style scoped>
.range-editor { margin-top: 20px; color: var(--c-text); }
h3 { margin: 0 0 6px; font-size: 14px; }
.range-hint, .range-empty, .range-count { color: var(--c-subtext0); font-size: 12px; margin: 6px 0; line-height: 1.5; }
.range-area { margin: 12px 0; padding: 10px 12px; border: 1px solid var(--c-surface1); border-radius: 6px; min-width: 0; }
legend { padding: 0 5px; font-size: 13px; font-weight: 600; }
legend span { color: var(--c-subtext0); margin-left: 6px; font-size: 12px; font-weight: 400; }
.range-row + .range-row { border-top: 1px solid var(--c-surface1); padding-top: 10px; margin-top: 10px; }
.range-fields { display: grid; grid-template-columns: 1fr 1fr 1fr 1.4fr auto; gap: 8px; align-items: end; }
label { display: flex; flex-direction: column; gap: 5px; min-width: 0; font-size: 12px; color: var(--c-subtext0); }
input, select, button { box-sizing: border-box; min-height: 34px; font: inherit; border-radius: 4px; color: var(--c-text); border: 1px solid var(--c-surface1); background: var(--c-surface0); }
input, select { width: 100%; padding: 6px; min-width: 0; }
button { cursor: pointer; padding: 6px 10px; font-size: 12px; }
button:hover { background: var(--c-surface1); }
button:disabled { opacity: .6; cursor: default; }
input:focus-visible, select:focus-visible, button:focus-visible { outline: 2px solid var(--c-blue); outline-offset: 2px; }
.range-error { color: var(--c-red); font-size: 12px; margin: 6px 0; }
.range-total { font-size: 13px; margin-bottom: 0; }
@media (max-width: 650px) { .range-fields { grid-template-columns: 1fr 1fr; } .range-remove { justify-self: start; } }
</style>
