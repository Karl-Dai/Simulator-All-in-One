<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from '../i18n'
import QualityLegend from './QualityLegend.vue'

export interface QualityBits {
  ov: boolean
  bl: boolean
  sb: boolean
  nt: boolean
  iv: boolean
}

const props = withDefaults(defineProps<{
  quality: QualityBits
  /** 可编辑时徽章可点击切换(子站);只读时仅展示(主站)。 */
  editable?: boolean
  /** OV 仅测量类适用;非测量类传 false 隐藏 OV 徽章。 */
  showOv?: boolean
  /** 是否渲染 (?) 图例按钮(表格行内场景关闭)。 */
  showHelp?: boolean
  /** 紧凑模式:只渲染置位的品质位,全部正常时显示 OK(用于窄表格列)。 */
  compact?: boolean
}>(), { editable: false, showOv: true, showHelp: true, compact: false })

const emit = defineEmits<{ (e: 'toggle', bit: keyof QualityBits): void }>()
const { t } = useI18n()

// 协议固定顺序与字母,语言无关。
const ALL_BITS: { key: keyof QualityBits; letter: string }[] = [
  { key: 'iv', letter: 'IV' },
  { key: 'nt', letter: 'NT' },
  { key: 'sb', letter: 'SB' },
  { key: 'bl', letter: 'BL' },
  { key: 'ov', letter: 'OV' },
]
// 适用的位(按 showOv 过滤 OV)
const applicable = computed(() => (props.showOv ? ALL_BITS : ALL_BITS.filter((b) => b.key !== 'ov')))
// 实际渲染的徽章:紧凑模式只留置位的
const bits = computed(() =>
  props.compact ? applicable.value.filter((b) => props.quality[b.key]) : applicable.value,
)
const anyLit = computed(() => applicable.value.some((b) => props.quality[b.key]))

function onBadge(key: keyof QualityBits) {
  if (props.editable) emit('toggle', key)
}
</script>

<template>
  <div class="quality-indicator">
    <span class="q-badges">
      <button
        v-for="b in bits"
        :key="b.key"
        type="button"
        class="q-badge"
        :class="[`q-${b.key}`, { lit: quality[b.key], editable }]"
        :disabled="!editable"
        :title="t(`quality.bits.${b.key}.name`)"
        @click="onBadge(b.key)"
      >{{ b.letter }}</button>
      <span v-if="compact && !anyLit" class="q-ok">OK</span>
    </span>
    <QualityLegend v-if="showHelp" />
  </div>
</template>

<style scoped>
.quality-indicator {
  position: relative;
  display: inline-flex;
  align-items: center;
  gap: 6px;
}
.q-badges {
  display: inline-flex;
  gap: 3px;
}
.q-ok {
  font: 600 11px/1 ui-monospace, monospace;
  color: var(--c-green);
}
.q-badge {
  font: 600 11px/1 ui-monospace, monospace;
  padding: 2px 4px;
  border-radius: 3px;
  border: 1px solid var(--c-surface1);
  background: var(--c-surface0);
  color: var(--c-overlay0);
  cursor: default;
}
.q-badge.editable {
  cursor: pointer;
}
.q-badge.editable:hover {
  border-color: var(--c-overlay0);
}
/* 置位高亮:IV 最严重用红,其余用桃色警示 */
.q-badge.lit {
  color: var(--c-crust);
  border-color: transparent;
  background: var(--c-peach);
}
.q-badge.q-iv.lit {
  background: var(--c-red);
}
</style>
