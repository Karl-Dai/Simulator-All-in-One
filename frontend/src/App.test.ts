import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import App from "./App.vue";
import type { Snapshot, SuiteAPI } from "./types";

let api: SuiteAPI;
let state: Snapshot;
const mounted: ReturnType<typeof mount>[] = [];
function render() {
  const app = mount(App);
  mounted.push(app);
  return app;
}

beforeEach(() => {
  state = {
    version: "0.1.0",
    platform: "darwin",
    architecture: "arm64",
    warning: "",
    preferences: {
      schemaVersion: 1,
      theme: "system",
      language: "zh-CN",
      favorites: [],
      recent: {},
    },
    roles: [
      ["iec104.master", "iec104", "主站", "IEC104Master"],
      ["iec104.slave", "iec104", "子站", "IEC104Slave"],
      ["modbus.master", "modbus", "主站", "ModbusMaster"],
      ["modbus.slave", "modbus", "从站", "ModbusSlave"],
      ["pmu.master", "pmu", "主站", "PmuSim"],
      ["pmu.sub", "pmu", "子站", "PmuSub"],
      ["opcua.master", "opcua", "客户端", "OPCUAMaster"],
      ["opcua.server", "opcua", "服务端", "OPCUAServer"],
    ].map(([id, protocol, name, product]) => ({
      id,
      protocol,
      name,
      product,
      version: "1.0.0",
      ready: true,
      error: "",
    })),
  };
  api = {
    Snapshot: vi.fn(async () => state),
    Launch: vi.fn(async (id) => ({ id, pid: 123, warning: "" })),
    SetFavorite: vi.fn(async (id, favorite) => ({
      ...state.preferences,
      favorites: favorite ? [id] : [],
    })),
    SetTheme: vi.fn(async (theme) => ({ ...state.preferences, theme })),
    SetLanguage: vi.fn(async (language) => ({
      ...state.preferences,
      language,
    })),
  };
  window.go = { main: { App: api } };
});
afterEach(() => {
  mounted.splice(0).forEach((app) => app.unmount());
  delete window.go;
  vi.clearAllMocks();
});

describe("complete suite launcher", () => {
  it("shows eight direct launch actions and no component download flow", async () => {
    const app = render();
    await flushPromises();
    expect(app.findAll(".launch-button")).toHaveLength(8);
    expect(app.text()).toContain("8 / 8 已就绪");
    expect(app.findAll("a")).toHaveLength(0);
    await app.get('[aria-label="打开 Modbus 主站"]').trigger("click");
    await flushPromises();
    expect(api.Launch).toHaveBeenCalledWith("modbus.master");
    expect(app.text()).toContain("提交启动请求");
  });
  it("favorites filter only the chosen role", async () => {
    const app = render();
    await flushPromises();
    await app.get('[aria-label="收藏 OPC UA 服务端"]').trigger("click");
    await flushPromises();
    const nav = app.findAll("nav button")[1];
    await nav.trigger("click");
    expect(app.findAll(".protocol-card")).toHaveLength(1);
    expect(
      app.findAll(".role-row").filter((row) => row.isVisible()),
    ).toHaveLength(1);
    expect(app.text()).toContain("OPCUAServer");
  });
  it("does not launch a missing or corrupt component", async () => {
    state.roles[0].ready = false;
    state.roles[0].error = "内置程序校验不匹配";
    const app = render();
    await flushPromises();
    await app.get('[aria-label="打开 IEC 104 主站"]').trigger("click");
    await flushPromises();
    expect(api.Launch).not.toHaveBeenCalled();
    expect(app.get('[role="alertdialog"]').text()).toContain(
      "内置程序校验不匹配",
    );
  });
  it("surfaces backend failures and restores the launch action", async () => {
    vi.mocked(api.Launch).mockRejectedValueOnce(new Error("系统拒绝启动"));
    const app = render();
    await flushPromises();
    const button = app.get('[aria-label="打开 PMU 主站"]');
    await button.trigger("click");
    await flushPromises();
    expect(app.get('[role="alertdialog"]').text()).toContain("系统拒绝启动");
    expect(button.attributes("disabled")).toBeUndefined();
  });
  it("persists the selected theme", async () => {
    const app = render();
    await flushPromises();
    await app.get(".about-link").trigger("click");
    const dark = app.findAll(".theme-options button")[2];
    await dark.trigger("click");
    await flushPromises();
    expect(api.SetTheme).toHaveBeenCalledWith("dark");
    expect(document.documentElement.dataset.theme).toBe("dark");
  });
  it("switches the interface language and persists it", async () => {
    const app = render();
    await flushPromises();
    await app.get(".lang-button").trigger("click");
    await flushPromises();
    expect(api.SetLanguage).toHaveBeenCalledWith("en");
    expect(app.text()).toContain("All Simulators");
    expect(app.get('[aria-label="Open Modbus Master"]')).toBeTruthy();
    await app.get(".lang-button").trigger("click");
    await flushPromises();
    expect(api.SetLanguage).toHaveBeenCalledWith("auto");
    await app.get(".lang-button").trigger("click");
    await flushPromises();
    expect(api.SetLanguage).toHaveBeenCalledWith("zh-CN");
    expect(app.text()).toContain("全部模拟器");
  });
  it("resolves auto language from the system locale", async () => {
    state.preferences.language = "auto";
    vi.spyOn(navigator, "language", "get").mockReturnValue("en-US");
    const app = render();
    await flushPromises();
    expect(app.text()).toContain("All Simulators");
    expect(document.documentElement.lang).toBe("en");
  });
});
