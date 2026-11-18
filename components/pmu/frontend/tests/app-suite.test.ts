import { afterEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/api/app", () => ({ getVersion: async () => "0.13.2" }));
import App from "../src/App.vue";

function render() {
  return mount(App, {
    global: { stubs: { ConfigInfoPanel: true, DataTablePanel: true, UpdateDialog: true } },
  });
}

afterEach(() => {
  vi.unstubAllEnvs();
  invoke.mockReset();
});

describe("suite update ownership", () => {
  it("does not check or offer per-app updates in a suite build", async () => {
    vi.stubEnv("VITE_SIMLAB_SUITE", "1");
    const wrapper = render();
    await flushPromises();
    expect(invoke).not.toHaveBeenCalled();
    expect(wrapper.find(".check-btn").exists()).toBe(false);
    expect(wrapper.findComponent({ name: "UpdateDialog" }).exists()).toBe(false);
    wrapper.unmount();
  });

  it("retains automatic and manual checks in the standalone build", async () => {
    vi.stubEnv("VITE_SIMLAB_SUITE", "");
    invoke.mockResolvedValue(null);
    const wrapper = render();
    await flushPromises();
    expect(invoke).toHaveBeenCalledWith("check_for_update", { force: false });
    await wrapper.get(".check-btn").trigger("click");
    await flushPromises();
    expect(invoke).toHaveBeenCalledWith("check_for_update", { force: true });
    wrapper.unmount();
  });
});
