<script setup lang="ts">
import { ref, onBeforeUnmount } from 'vue'
import { useI18n } from '../i18n'

const { t } = useI18n()

// 协议固定顺序与字母,语言无关。
const BITS = ['iv', 'nt', 'sb', 'bl', 'ov'] as const
const LETTER: Record<(typeof BITS)[number], string> = {
  iv: 'IV', nt: 'NT', sb: 'SB', bl: 'BL', ov: 'OV',
}

const open = ref(false)
const btn = ref<HTMLElement | null>(null)
// popover 用 fixed 定位(teleport 到 body),从按钮 rect 计算,右对齐避免溢出右缘。
const pos = ref({ top: 0, right: 0 })

function place() {
  const el = btn.value
  if (!el) return
  const r = el.getBoundingClientRect()
  pos.value = {
    top: r.bottom + 4,
    right: Math.max(8, window.innerWidth - r.right),
  }
}

function open_() {
  place()
  open.value = true
  // 推迟到下一帧再挂监听,避免当前这次点击立即触发 outside 关闭。
  requestAnimationFrame(() => {
    document.addEventListener('pointerdown', onDocPointer, true)
    document.addEventListener('keydown', onKey, true)
    window.addEventListener('scroll', close, true)
    window.addEventListener('resize', close, true)
  })
}

function close() {
  if (!open.value) return
  open.value = false
  document.removeEventListener('pointerdown', onDocPointer, true)
  document.removeEventListener('keydown', onKey, true)
  window.removeEventListener('scroll', close, true)
  window.removeEventListener('resize', close, true)
}

function toggle() {
  open.value ? close() : open_()
}

function onDocPointer(e: Event) {
  const target = e.target as HTMLElement
  if (btn.value?.contains(target)) return
  if (target.closest?.('.q-legend')) return // 点在 popover 内不关
  close()
}
function onKey(e: KeyboardEvent) {
  if (e.key === 'Escape') close()
}

onBeforeUnmount(close)
</script>

<template>
  <button
    ref="btn"
    type="button"
    class="q-help"
    :aria-label="t('quality.legendTitle')"
    @click.stop="toggle"
  >?</button>
  <Teleport to="body">
    <div
      v-if="open"
      class="q-legend"
      :style="{ top: pos.top + 'px', right: pos.right + 'px' }"
    >
      <div class="q-legend-title">{{ t('quality.legendTitle') }}</div>
      <div v-for="b in BITS" :key="b" class="q-legend-row">
        <span class="q-legend-letter">{{ LETTER[b] }}</span>
        <span class="q-legend-name">{{ t(`quality.bits.${b}.name`) }}</span>
        <span class="q-legend-desc">{{ t(`quality.bits.${b}.desc`) }}</span>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.q-help {
  width: 16px;
  height: 16px;
  line-height: 14px;
  border-radius: 50%;
  border: 1px solid var(--c-surface2);
  background: var(--c-surface0);
  color: var(--c-subtext0);
  font-size: 11px;
  cursor: pointer;
  padding: 0;
}
.q-help:hover {
  border-color: var(--c-blue);
  color: var(--c-blue);
}
/* teleport 到 body,fixed 定位 → 不受任何祖先 overflow/stacking 裁剪 */
.q-legend {
  position: fixed;
  z-index: 1000;
  min-width: 280px;
  padding: 8px 10px;
  background: var(--c-mantle);
  border: 1px solid var(--c-surface1);
  border-radius: 6px;
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.4);
}
.q-legend-title {
  font-size: 11px;
  font-weight: 600;
  color: var(--c-subtext1);
  margin-bottom: 6px;
  white-space: nowrap;
}
.q-legend-row {
  display: grid;
  grid-template-columns: 28px 56px 1fr;
  gap: 6px;
  align-items: baseline;
  font-size: 12px;
  padding: 2px 0;
}
.q-legend-letter {
  font: 600 11px/1 ui-monospace, monospace;
  color: var(--c-peach);
}
.q-legend-name {
  color: var(--c-text);
}
.q-legend-desc {
  color: var(--c-subtext0);
  font-size: 11px;
}
</style>
