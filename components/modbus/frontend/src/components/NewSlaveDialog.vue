<script setup lang="ts">
import { vModal } from 'shared-frontend'
import { computed, ref, watch } from 'vue'
import * as App from '../../wailsjs/go/main/App'
import { useI18n, showAlert } from 'shared-frontend'

import RegisterRangeEditor from './RegisterRangeEditor.vue'
import { rangesValid, type RegisterRange } from '../utils/registerRanges'

const { t } = useI18n()

interface Props { show: boolean; connectionId: string | null }
const props = defineProps<Props>()
const emit = defineEmits<{ (e: 'close'): void; (e: 'created'): void }>()

const slaveId = ref('2')
const slaveName = ref('')
const initMode = ref('zero')
const registerRanges = ref<RegisterRange[]>([])
const isSaving = ref(false)
const canSubmit = computed(() => !isSaving.value && rangesValid(registerRanges.value))

watch(() => props.show, (visible) => {
  if (!visible) return
  slaveId.value = '2'
  slaveName.value = ''
  initMode.value = 'zero'
  registerRanges.value = []
})

async function submit() {
  if (!canSubmit.value) return
  if (!props.connectionId) return
  const id = Number(slaveId.value)
  if (!id || id < 1 || id > 247) {
    await showAlert(t('errors.invalidSlaveId'))
    return
  }
  isSaving.value = true
  try {
    await App.AddDevice({ ConnectionID: props.connectionId, SlaveID: id, Name: slaveName.value.trim(), InitMode: initMode.value, RegisterRanges: registerRanges.value })
    emit('created')
    emit('close')
  } catch (e) {
    await showAlert(String(e))
  } finally {
    isSaving.value = false
  }
}
</script>

<template>
  <Teleport to="body">
    <div v-if="show" v-modal="() => { if (!isSaving) emit('close') }" class="modal-overlay" aria-labelledby="new-slave-title" @pointerdown.self.left="!isSaving && emit('close')">
      <div class="modal-box" :aria-busy="isSaving">
        <fieldset class="creation-fields" :disabled="isSaving">
        <div id="new-slave-title" class="modal-title">{{ t('toolbar.newSlave') }}</div>
        <div class="modal-field">
          <label for="new-slave-slaveId">{{ t('dialog.slaveId') }}</label>
          <input id="new-slave-slaveId" v-model="slaveId" type="number" min="1" max="247" @keyup.enter="submit" />
        </div>
        <div class="modal-field">
          <label for="new-slave-slaveName">{{ t('dialog.slaveName') }}</label>
          <input id="new-slave-slaveName" v-model="slaveName" type="text" :placeholder="t('dialog.slaveNamePlaceholder')" @keyup.enter="submit" />
        </div>
        <div class="modal-field">
          <label>{{ t('dialog.initValue') }}</label>
          <div class="radio-group">
            <label class="radio-label"><input type="radio" v-model="initMode" value="zero" /> {{ t('dialog.initZero') }}</label>
            <label class="radio-label"><input type="radio" v-model="initMode" value="random" /> {{ t('dialog.initRandom') }}</label>
          </div>
        </div>
        <RegisterRangeEditor v-model="registerRanges" :disabled="isSaving" />
        </fieldset>
        <div class="modal-actions">
          <button class="modal-btn cancel" :disabled="isSaving" @click="emit('close')">{{ t('common.cancel') }}</button>
          <button class="modal-btn confirm" :disabled="!canSubmit" @click="submit">{{ t(isSaving ? 'ranges.creating' : 'common.ok') }}</button>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.modal-overlay { position: fixed; inset: 0; background: rgba(0,0,0,0.5); display: flex; align-items: center; justify-content: center; z-index: 1000; }
.modal-box { background: var(--c-base); border: 1px solid var(--c-surface1); border-radius: 8px; padding: 20px; width: min(760px, calc(100vw - 32px)); max-height: 90vh; overflow-y: auto; box-sizing: border-box; box-shadow: 0 8px 24px rgba(0,0,0,0.5); }
.creation-fields { border: 0; padding: 0; margin: 0; min-width: 0; }
.modal-btn:disabled { opacity: .5; cursor: default; }
.modal-title { font-size: 14px; font-weight: 600; color: var(--c-text); margin-bottom: 16px; }
.modal-field { margin-bottom: 14px; }
.modal-field label { display: block; font-size: 12px; color: var(--c-subtext0); margin-bottom: 6px; }
.modal-field input[type="number"], .modal-field input[type="text"] { width: 100%; padding: 6px 10px; background: var(--c-surface0); border: 1px solid var(--c-surface1); border-radius: 4px; color: var(--c-text); font-size: 13px; outline: none; box-sizing: border-box; }
.modal-field input:focus { border-color: var(--c-blue); }
.radio-group { display: flex; gap: 16px; }
.radio-label { display: flex; align-items: center; gap: 6px; font-size: 13px; color: var(--c-text); cursor: pointer; }
.radio-label input[type="radio"] { accent-color: var(--c-blue); }
.modal-actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 18px; }
.modal-btn { padding: 6px 16px; border: none; border-radius: 4px; font-size: 12px; cursor: pointer; }
.modal-btn.cancel { background: var(--c-surface0); color: var(--c-subtext0); }
.modal-btn.cancel:hover { background: var(--c-surface1); }
.modal-btn.confirm { background: var(--c-blue); color: var(--c-base); font-weight: 600; }
.modal-btn.confirm:hover { background: var(--c-sapphire); }
</style>
