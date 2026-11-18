import { afterEach, expect, it, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { useI18n } from '../src/i18n'
import NewConnectionDialog from '../../master-frontend/src/components/NewConnectionDialog.vue'
import * as App from '../../master-frontend/wailsjs/go/main/App'
import type { masterstate, project } from '../../master-frontend/wailsjs/go/models'
import { defaultCommunicationOptions } from '../../master-frontend/src/communication'

vi.mock('../../master-frontend/wailsjs/go/main/App', () => ({
  GetSettings: vi.fn(),
  ListSerialPorts: vi.fn().mockResolvedValue([]),
  OpenFileDialog: vi.fn(),
  UpdateConnection: vi.fn(),
  CreateConnection: vi.fn(),
}))
let wrapper: ReturnType<typeof mount> | undefined
afterEach(() => { wrapper?.unmount(); vi.resetAllMocks(); document.body.innerHTML = '' })

function tlsSettings(tls: project.ClientTLS): masterstate.Settings {
  const defaults = defaultCommunicationOptions()
  return {
    Transport: { Type: 'tcp_tls', Host: '192.0.2.1', Port: 502, TLS: tls },
    SlaveID: 1, TimeoutMS: 3000, Requests: defaults.requests, Reconnect: defaults.reconnect,
  }
}

function checkbox(label: string): HTMLInputElement {
  return [...document.querySelectorAll('label')]
    .find(node => node.textContent?.trim() === label)!.querySelector<HTMLInputElement>('input')!
}

function nameCheck(): HTMLInputElement {
  return document.querySelector<HTMLInputElement>('#tls-verify-server-name')!
}

function save() {
  document.querySelector<HTMLFormElement>('form')!.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }))
}

it('requires an explicit per-connection opt-in and resets it for the next connection', async () => {
  useI18n().setLocale('zh-CN')
  vi.mocked(App.CreateConnection).mockResolvedValue({ id: 'qa' } as never)
  wrapper = mount(NewConnectionDialog, { attachTo: document.body, props: { show: true } })
  const enableTls = () => {
    const label = [...document.querySelectorAll('label')].find(node => node.textContent?.trim() === '启用 TLS')!
    label.querySelector<HTMLInputElement>('input')!.click()
  }
  enableTls()
  await flushPromises()
  let checkbox = document.querySelector<HTMLInputElement>('#tls-compatibility-hint')!.previousElementSibling!.querySelector<HTMLInputElement>('input')!
  expect(checkbox.checked).toBe(false)
  expect(document.querySelector('#tls-compatibility-hint')!.textContent).toContain('不验证服务器身份')
  checkbox.click()
  await flushPromises()
  const create = [...document.querySelectorAll<HTMLButtonElement>('button')].find(button => button.textContent === '创建')!
  create.click()
  await flushPromises()
  expect(App.CreateConnection).toHaveBeenCalledWith(expect.objectContaining({
    Transport: expect.objectContaining({
      Type: 'tcp_tls',
      TLS: expect.objectContaining({ Enabled: true, AcceptInvalidCerts: true }),
    }),
  }))
  await wrapper.setProps({ show: false })
  await wrapper.setProps({ show: true })
  enableTls()
  await flushPromises()
  checkbox = document.querySelector<HTMLInputElement>('#tls-compatibility-hint')!.previousElementSibling!.querySelector<HTMLInputElement>('input')!
  expect(checkbox.checked).toBe(false)
})

it('defaults name matching off for new TLS connections while keeping certificate checks on', async () => {
  useI18n().setLocale('en-US')
  wrapper = mount(NewConnectionDialog, { attachTo: document.body, props: { show: true } })
  checkbox('Enable TLS').click()
  await flushPromises()
  expect(nameCheck().checked).toBe(false)
  expect(nameCheck().disabled).toBe(false)
  expect(document.querySelector('#tls-server-name-hint')!.textContent).toContain('CA certificate chain')
  save()
  await flushPromises()
  expect(App.CreateConnection).toHaveBeenCalledWith(expect.objectContaining({
    Transport: expect.objectContaining({ TLS: expect.objectContaining({ VerifyServerName: false, AcceptInvalidCerts: false }) }),
  }))
})

it('loads older TLS settings as name matching off, saves opt-in, and restores it on reopening', async () => {
  useI18n().setLocale('zh-CN')
  const original = tlsSettings({ Enabled: true, CAFile: '/certs/ca.pem', AcceptInvalidCerts: false })
  vi.mocked(App.GetSettings).mockResolvedValue(original)
  wrapper = mount(NewConnectionDialog, { attachTo: document.body, props: { show: true, connectionId: 'existing' } })
  await flushPromises()
  expect(nameCheck().checked).toBe(false)
  nameCheck().click()
  await flushPromises()
  save()
  await flushPromises()
  const [, saved] = vi.mocked(App.UpdateConnection).mock.calls[0]!
  expect(saved.Transport.TLS).toMatchObject({ CAFile: '/certs/ca.pem', VerifyServerName: true, AcceptInvalidCerts: false })
  vi.mocked(App.GetSettings).mockResolvedValue(saved)
  await wrapper.setProps({ show: false })
  await wrapper.setProps({ show: true })
  await flushPromises()
  expect(nameCheck().checked).toBe(true)

  await wrapper.setProps({ show: false })
  await wrapper.setProps({ show: true, connectionId: undefined })
  checkbox('启用 TLS').click()
  await flushPromises()
  expect(nameCheck().checked).toBe(false)
})

it('disables name matching when all checks are skipped without losing the saved choice', async () => {
  useI18n().setLocale('zh-CN')
  vi.mocked(App.GetSettings).mockResolvedValue(tlsSettings({ Enabled: true, VerifyServerName: true, AcceptInvalidCerts: false }))
  wrapper = mount(NewConnectionDialog, { attachTo: document.body, props: { show: true, connectionId: 'existing' } })
  await flushPromises()
  const skipAll = checkbox('兼容旧设备证书（跳过证书及地址校验）')
  skipAll.click()
  await flushPromises()
  expect(nameCheck().checked).toBe(true)
  expect(nameCheck().disabled).toBe(true)
  expect(document.querySelector('#tls-server-name-hint')).toBeNull()
  expect(nameCheck().getAttribute('aria-describedby')).toBe('tls-compatibility-hint')
  save()
  await flushPromises()
  expect(App.UpdateConnection).toHaveBeenCalledWith('existing', expect.objectContaining({
    Transport: expect.objectContaining({ TLS: expect.objectContaining({ VerifyServerName: true, AcceptInvalidCerts: true }) }),
  }))
  skipAll.click()
  await flushPromises()
  expect(nameCheck().disabled).toBe(false)
  expect(nameCheck().checked).toBe(true)
})
