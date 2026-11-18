// quality-descriptor 验证项 5.4:品质开关渲染、OV 在 SP/DP 隐藏、popover 文案。
import { describe, it, expect } from 'vitest'
import { mount } from '@vue/test-utils'
import QualityIndicator from '@shared/components/QualityIndicator.vue'
import QualityLegend from '@shared/components/QualityLegend.vue'
import { useI18n } from '@shared/i18n'

type Bits = { ov: boolean; bl: boolean; sb: boolean; nt: boolean; iv: boolean }
function q(over: Partial<Bits> = {}): Bits {
  return { ov: false, bl: false, sb: false, nt: false, iv: false, ...over }
}

describe('QualityIndicator', () => {
  it('measured 类型显示全部 5 个品质徽章', () => {
    const w = mount(QualityIndicator, { props: { quality: q(), showOv: true } })
    const letters = w.findAll('.q-badge').map((b) => b.text())
    expect(letters).toEqual(['IV', 'NT', 'SB', 'BL', 'OV'])
  })

  it('SP/DP(showOv=false)隐藏 OV 徽章', () => {
    const w = mount(QualityIndicator, { props: { quality: q(), showOv: false } })
    const letters = w.findAll('.q-badge').map((b) => b.text())
    expect(letters).toEqual(['IV', 'NT', 'SB', 'BL'])
    expect(letters).not.toContain('OV')
  })

  it('置位的品质位高亮(lit)', () => {
    const w = mount(QualityIndicator, { props: { quality: q({ nt: true }), showOv: true } })
    const nt = w.findAll('.q-badge').find((b) => b.text() === 'NT')!
    expect(nt.classes()).toContain('lit')
  })

  it('可编辑时点击徽章 emit toggle', async () => {
    const w = mount(QualityIndicator, { props: { quality: q(), editable: true, showOv: true } })
    const nt = w.findAll('.q-badge').find((b) => b.text() === 'NT')!
    await nt.trigger('click')
    expect(w.emitted('toggle')?.[0]).toEqual(['nt'])
  })

  it('只读时点击徽章不 emit', async () => {
    const w = mount(QualityIndicator, { props: { quality: q(), editable: false, showOv: true } })
    const nt = w.findAll('.q-badge').find((b) => b.text() === 'NT')!
    await nt.trigger('click')
    expect(w.emitted('toggle')).toBeUndefined()
  })

  it('点击 ? 图例 teleport 到 body(不被祖先 overflow 裁剪)并显示中文释义', async () => {
    useI18n().setLocale('zh-CN')
    const w = mount(QualityIndicator, { props: { quality: q(), showOv: true } })
    // 未点击时 body 无图例
    expect(document.body.querySelector('.q-legend')).toBeNull()
    await w.find('.q-help').trigger('click')
    // 关键:图例渲染到 document.body,而不是留在(可能被裁剪的)组件子树里
    const legend = document.body.querySelector('.q-legend')
    expect(legend).not.toBeNull()
    expect(w.element.contains(legend)).toBe(false) // 已 teleport 出组件子树
    expect(legend!.textContent).toContain('无效')   // IV
    expect(legend!.textContent).toContain('非现时') // NT
    w.unmount()
    expect(document.body.querySelector('.q-legend')).toBeNull() // 卸载后清理
  })
})

describe('QualityLegend(独立图例,用于表头)', () => {
  it('点击 ? teleport 到 body,再点 ? 关闭', async () => {
    useI18n().setLocale('zh-CN')
    const w = mount(QualityLegend)
    expect(document.body.querySelector('.q-legend')).toBeNull()
    await w.find('.q-help').trigger('click')
    const legend = document.body.querySelector('.q-legend')
    expect(legend).not.toBeNull()
    expect(legend!.textContent).toContain('溢出') // OV
    await w.find('.q-help').trigger('click') // 再点关闭
    expect(document.body.querySelector('.q-legend')).toBeNull()
    w.unmount()
  })
})
