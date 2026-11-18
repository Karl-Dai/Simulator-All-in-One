import { beforeEach, describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import { nextTick } from 'vue'
import { useI18n } from '@shared/i18n'
import DoublePointLegend from '@shared/components/DoublePointLegend.vue'

describe('DoublePointLegend localization', () => {
  beforeEach(() => {
    useI18n().setLocale('en-US')
  })

  it('shows all DPI states from the Value-header help button', async () => {
    const wrapper = mount(DoublePointLegend, {
      global: { stubs: { Teleport: true } },
    })

    await wrapper.find('.dp-help').trigger('click')
    await nextTick()
    expect(wrapper.text()).toContain('DPI 0')
    expect(wrapper.text()).toContain('Open')
    expect(wrapper.text()).toContain('Closed')
    expect(wrapper.text()).toContain('Indeterminate')

    useI18n().setLocale('zh-CN')
    await nextTick()
    expect(wrapper.text()).toContain('分闸')
    expect(wrapper.text()).toContain('合闸')
    expect(wrapper.text()).toContain('不确定')
  })
})
