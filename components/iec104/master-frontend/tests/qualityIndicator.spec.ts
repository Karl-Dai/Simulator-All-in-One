// quality-descriptor 验证项 6.3:主站多位徽章渲染 + 表格 compact 模式。
import { describe, it, expect } from 'vitest'
import { mount } from '@vue/test-utils'
import QualityIndicator from '@shared/components/QualityIndicator.vue'

type Bits = { ov: boolean; bl: boolean; sb: boolean; nt: boolean; iv: boolean }
function q(over: Partial<Bits> = {}): Bits {
  return { ov: false, bl: false, sb: false, nt: false, iv: false, ...over }
}

describe('QualityIndicator (master)', () => {
  it('多位置位同时高亮(quality_nt + sb)', () => {
    const w = mount(QualityIndicator, { props: { quality: q({ nt: true, sb: true }), showOv: true } })
    const lit = w.findAll('.q-badge.lit').map((b) => b.text()).sort()
    expect(lit).toEqual(['NT', 'SB'])
  })

  it('表格 compact 模式只渲染置位的位且无 ? 按钮', () => {
    const w = mount(QualityIndicator, {
      props: { quality: q({ nt: true, sb: true }), compact: true, showHelp: false },
    })
    expect(w.findAll('.q-badge').map((b) => b.text())).toEqual(['NT', 'SB'])
    expect(w.find('.q-help').exists()).toBe(false)
  })

  it('compact 全部正常时显示 OK', () => {
    const w = mount(QualityIndicator, {
      props: { quality: q(), compact: true, showHelp: false },
    })
    expect(w.findAll('.q-badge').length).toBe(0)
    expect(w.find('.q-ok').exists()).toBe(true)
  })
})
