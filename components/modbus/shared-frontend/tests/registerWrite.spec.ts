import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { useI18n } from '../src/i18n'
import { encodeRegisterWrite, type RegisterWriteType } from '../src/composables/useValueFormat'
import WriteDialog from '../../master-frontend/src/components/WriteDialog.vue'
import * as App from '../../master-frontend/wailsjs/go/main/App'

vi.mock('../../master-frontend/wailsjs/go/main/App', () => ({
  WriteSingleRegisterSlave: vi.fn(), WriteSingleCoilSlave: vi.fn(),
  WriteMultipleRegistersSlave: vi.fn(), WriteMultipleCoilsSlave: vi.fn(),
}))
let wrapper: ReturnType<typeof mount>
const button = () => wrapper.get<HTMLButtonElement>('.btn-primary')
const address = () => wrapper.get('input[type="number"]')
const values = () => wrapper.get('textarea')
const typeSelect = () => wrapper.findAll('select')[1]!
const orderSelect = () => wrapper.findAll('select')[2]!
async function select(type: RegisterWriteType, order?: string) {
  await typeSelect().setValue(type)
  if (order) await orderSelect().setValue(order)
}
beforeEach(async () => {
  vi.resetAllMocks()
  useI18n().setLocale('zh-CN')
  wrapper = mount(WriteDialog, {
    props: { show: true, connectionId: 'connection', slaveId: 7 },
    global: { stubs: { teleport: true } },
  })
  await wrapper.get('select').setValue('write_multiple_registers')
})
afterEach(() => wrapper.unmount())

describe('FC16 typed writes', () => {
  it('offers all types and changes byte orders with the value width', async () => {
    expect(typeSelect().findAll('option').map(o => o.element.value))
      .toEqual(['raw', 'uint16', 'int16', 'uint32', 'int32', 'uint64', 'int64', 'float32', 'float64'])
    for (const [type, orders] of [
      ['int16', ['AB', 'BA']], ['float32', ['ABCD', 'CDAB', 'BADC', 'DCBA']],
      ['float64', ['ABCDEFGH', 'GHEFCDAB', 'BADCFEHG', 'HGFEDCBA']],
    ] as const) {
      await select(type)
      expect(orderSelect().findAll('option').map(o => o.element.value)).toEqual(orders)
      expect(orderSelect().element.value).toBe(orders[0])
    }
  })

  it.each<[RegisterWriteType, string, string, number[]]>([
    ['raw', 'AB', '0x1234, 65535', [0x1234, 0xffff]],
    ['uint16', 'BA', '0x1234', [0x3412]],
    ['int16', 'AB', '-32768, 32767', [0x8000, 0x7fff]],
    ['int32', 'CDAB', '-2147483648', [0, 0x8000]],
    ['uint32', 'BADC', '0x12345678', [0x3412, 0x7856]],
    ['int64', 'ABCDEFGH', '-9223372036854775808', [0x8000, 0, 0, 0]],
    ['uint64', 'ABCDEFGH', '9007199254740993', [0x20, 0, 0, 1]],
    ['uint64', 'HGFEDCBA', '18446744073709551615', [0xffff, 0xffff, 0xffff, 0xffff]],
    ['float32', 'DCBA', '1', [0, 0x803f]],
    ['float64', 'GHEFCDAB', '-2.5', [0, 0, 0, 0xc004]],
  ])('previews and sends %s in %s order', async (type, order, input, expected) => {
    await select(type, order)
    await address().setValue('100')
    await values().setValue(input)
    expect(wrapper.get('[role="status"]').text()).toContain(`100–${100 + expected.length - 1}`)
    const words = wrapper.findAll('.preview-table tbody tr').flatMap(row => row.findAll('td')[2]!.text().split(' '))
    expect(words).toEqual(expected.map(word => '0x' + word.toString(16).toUpperCase().padStart(4, '0')))
    await button().trigger('click')
    await flushPromises()
    expect(App.WriteMultipleRegistersSlave).toHaveBeenCalledExactlyOnceWith('connection', 7, 100, expected)
    expect(wrapper.emitted('close')).toHaveLength(1)
  })

  it.each<[RegisterWriteType, string]>([
    ['raw', ''], ['raw', '1,,2'], ['raw', '1,'], ['raw', '-1'], ['raw', '65536'],
    ['int16', '-32769'], ['int16', '32768'], ['uint32', '4294967296'], ['int32', '2147483648'],
    ['int64', '-9223372036854775809'], ['int64', '9223372036854775808'], ['uint64', '18446744073709551616'],
    ['uint64', '1.5'], ['uint64', '1e3'], ['float32', '3.5e38'], ['float32', '12abc'],
    ['float64', 'Infinity'], ['float64', 'NaN'], ['float64', '1e309'],
  ])('blocks invalid %s input %j', async (type, input) => {
    await select(type)
    await values().setValue(input)
    expect(button().element.disabled).toBe(true)
    expect(wrapper.get('[role="alert"]').text()).not.toBe('')
    await button().trigger('click')
    expect(App.WriteMultipleRegistersSlave).not.toHaveBeenCalled()
    expect(wrapper.emitted('close')).toBeUndefined()
  })

  it.each(['-1', '0.5', '', '65536'])('blocks invalid address %j', async input => {
    await values().setValue('1')
    await address().setValue(input)
    expect(button().element.disabled).toBe(true)
    expect(address().attributes('aria-invalid')).toBe('true')
    await button().trigger('click')
    expect(App.WriteMultipleRegistersSlave).not.toHaveBeenCalled()
  })

  it('accepts pasted separators and advances addresses by the type width', async () => {
    await select('float32')
    await values().setValue('1，2\n3 4')
    expect(wrapper.findAll('.preview-table tbody tr').map(row => row.findAll('td')[0]!.text())).toEqual(['0', '2', '4', '6'])
    await button().trigger('click')
    await flushPromises()
    expect(App.WriteMultipleRegistersSlave).toHaveBeenLastCalledWith('connection', 7, 0, [0x3f80, 0, 0x4000, 0, 0x4040, 0, 0x4080, 0])
  })

  it.each<[RegisterWriteType, number, number]>([['raw', 123, 65535], ['float32', 61, 65534], ['float64', 30, 65532]])
  ('enforces quantity and address boundaries for %s', async (type, limit, lastStart) => {
    await select(type)
    await values().setValue(Array(limit).fill('1').join(','))
    expect(button().element.disabled).toBe(false)
    await values().setValue(Array(limit + 1).fill('1').join(','))
    expect(button().element.disabled).toBe(true)
    expect(wrapper.get('[role="alert"]').text()).toContain('123')
    await values().setValue('1')
    await address().setValue(String(lastStart))
    expect(button().element.disabled).toBe(false)
    await address().setValue(String(lastStart + 1))
    expect(button().element.disabled).toBe(true)
    await button().trigger('click')
    expect(App.WriteMultipleRegistersSlave).not.toHaveBeenCalled()
  })

  it('prevents editing and duplicate submissions while the request is pending', async () => {
    await select('int64')
    await values().setValue('-1')
    let finish!: () => void
    vi.mocked(App.WriteMultipleRegistersSlave).mockImplementationOnce(() => new Promise(resolve => { finish = () => resolve(undefined) }))
    await button().trigger('click')
    expect(button().element.disabled).toBe(true)
    expect(wrapper.get('fieldset').element.disabled).toBe(true)
    await button().trigger('click')
    expect(App.WriteMultipleRegistersSlave).toHaveBeenCalledTimes(1)
    finish()
    await flushPromises()
    expect(wrapper.emitted('close')).toHaveLength(1)
  })

  it('resets on reopen, keeps FC06 working and translates labels and errors', async () => {
    await select('float64', 'HGFEDCBA')
    await wrapper.setProps({ show: false })
    await wrapper.setProps({ show: true })
    expect(wrapper.findAll('select')).toHaveLength(1)
    await wrapper.get('input[type="text"]').setValue('123')
    await button().trigger('click')
    await flushPromises()
    expect(App.WriteSingleRegisterSlave).toHaveBeenLastCalledWith('connection', 7, 0, 123)
    useI18n().setLocale('en-US')
    await wrapper.get('select').setValue('write_multiple_registers')
    expect(orderSelect().element.value).toBe('AB')
    await select('uint64')
    expect(typeSelect().text()).toContain('UInt64 (unsigned 64-bit)')
    await values().setValue('-1')
    expect(wrapper.get('[role="alert"]').text()).toContain('outside the uint64 range')
  })
})

describe('register encoding known vectors', () => {
  it.each([
    ['ABCD', [0x1234, 0x5678]], ['CDAB', [0x5678, 0x1234]],
    ['BADC', [0x3412, 0x7856]], ['DCBA', [0x7856, 0x3412]],
  ] as const)('encodes distinct UInt32 bytes in %s order', (order, expected) => {
    expect(encodeRegisterWrite('0x12345678', 'uint32', order)).toEqual(expected)
  })
  it.each([
    ['ABCDEFGH', [0x1234, 0x5678, 0x9abc, 0xdef0]], ['GHEFCDAB', [0xdef0, 0x9abc, 0x5678, 0x1234]],
    ['BADCFEHG', [0x3412, 0x7856, 0xbc9a, 0xf0de]], ['HGFEDCBA', [0xf0de, 0xbc9a, 0x7856, 0x3412]],
  ] as const)('preserves every UInt64 byte in %s order', (order, expected) => {
    expect(encodeRegisterWrite('0x123456789ABCDEF0', 'uint64', order)).toEqual(expected)
  })
  it('preserves integer precision, float sign and scientific notation', () => {
    expect(encodeRegisterWrite('9223372036854775807', 'int64', 'ABCDEFGH')).toEqual([0x7fff, 0xffff, 0xffff, 0xffff])
    expect(encodeRegisterWrite('-1', 'int64', 'ABCDEFGH')).toEqual([0xffff, 0xffff, 0xffff, 0xffff])
    expect(encodeRegisterWrite('-0', 'float32', 'ABCD')).toEqual([0x8000, 0])
    expect(encodeRegisterWrite('1.25e2', 'float64', 'ABCDEFGH')).toEqual([0x405f, 0x4000, 0, 0])
    expect(() => encodeRegisterWrite('1', 'uint16', 'ABCD')).toThrow('byte order')
  })
})
