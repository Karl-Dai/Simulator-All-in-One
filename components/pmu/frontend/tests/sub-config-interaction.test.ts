import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
import ConfigFormPanel from '../../frontend-sub/src/components/ConfigFormPanel.vue';
import { running } from '../../frontend-sub/src/composables/useSubEvents';
let wrapper: ReturnType<typeof mount>;
beforeEach(() => {
  window.__TAURI_INTERNALS__ = { invoke: async (cmd: string, args: unknown) => {
    if (cmd === 'plugin:store|load') return 1;
    if (cmd === 'plugin:store|keys') return [];
    return invoke(cmd, args);
  } } as any;
});
afterEach(() => { wrapper?.unmount(); running.value = false; invoke.mockReset(); });
describe('substation configuration controls', () => {
  it('rejects fractional counts without mutating the rows or throwing', async () => {
    wrapper = mount(ConfigFormPanel); await flushPromises();
    const count = wrapper.get('input[max="65535"][step="1"]');
    await count.setValue(0.5);
    expect(wrapper.findAll('.phasor-row')).toHaveLength(1);
    expect(wrapper.get('[role="alert"]').text()).not.toBe('');
    expect(wrapper.get('.actions button').element.disabled).toBe(true);
    await count.setValue(2);
    expect(wrapper.findAll('.phasor-row')).toHaveLength(2);
    expect(wrapper.find('[role="alert"]').exists()).toBe(false);
  });
  it('shows native request errors and restores controls for retry', async () => {
    invoke.mockRejectedValueOnce('Port already in use');
    wrapper = mount(ConfigFormPanel); await flushPromises();
    await wrapper.get('.actions button').trigger('click'); await flushPromises();
    expect(wrapper.get('[role="alert"]').text()).toContain('Port already in use');
    expect(running.value).toBe(false);
    expect(wrapper.get('fieldset').element.disabled).toBe(false);
    invoke.mockResolvedValueOnce(undefined);
    await wrapper.get('.actions button').trigger('click'); await flushPromises();
    expect(running.value).toBe(true);
    expect(wrapper.find('[role="alert"]').exists()).toBe(false);
  });
});
