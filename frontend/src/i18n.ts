import { ref } from "vue";
import type { Language } from "./types";

export type Locale = "zh-CN" | "en";

export const locale = ref<Locale>("en");

const messages: Record<Locale, Record<string, string>> = {
  "zh-CN": {
    "nav.all": "全部模拟器",
    "nav.favorites": "常用模拟器",
    "nav.main": "主导航",
    "theme.toggle": "切换外观，当前{label}（点击切换）",
    "theme.aria": "切换外观，当前{label}",
    "theme.system": "跟随系统",
    "theme.light": "浅色",
    "theme.dark": "深色",
    "readiness.checking": "正在检查套件",
    "readiness.ready": "{ready} / {total} 已就绪",
    "readiness.title": "重新检查内置套件",
    "lang.aria": "切换语言，当前{label}",
    "lang.auto": "跟随系统",
    "about.entry": "设置与关于",
    "settings.title": "设置",
    "settings.language": "语言",
    "workspace.independent": "在独立窗口中打开模拟器",
    "recent.label": "最近",
    "recent.section": "最近使用",
    "grid.section": "协议模拟器",
    "role.open": "打开",
    "role.repair": "需要修复",
    "role.openAria": "打开 {protocol} {role}",
    "role.favAria.on": "收藏 {protocol} {role}",
    "role.favAria.off": "取消收藏 {protocol} {role}",
    "roles.loading": "正在读取内置应用…",
    "roles.missing": "未能读取套件清单",
    "purpose.iec104": "远动与电力调度联调",
    "purpose.modbus": "PLC、仪表与寄存器联调",
    "purpose.pmu": "同步相量数据收发",
    "purpose.opcua": "节点浏览、订阅与变量仿真",
    "empty.title": "把常用工具放在手边",
    "empty.body": "在模拟器角色旁点击星标，即可收藏。",
    "empty.browse": "浏览全部模拟器",
    "toast.launched": "已向 {protocol} {role} 提交启动请求",
    "toast.close": "关闭提示",
    "error.title": "暂时无法完成操作",
    "error.help": "如果套件文件缺失或损坏，请重新运行完整安装包进行修复。",
    "error.dismiss": "知道了",
    "error.notReady": "内置程序不可用，请修复完整套件。",
    "about.title": "关于 Simulator All-in-One",
    "about.suite": "工业协议模拟器套件",
    "about.protocols": "{n} 种协议 · {m} 个角色",
    "about.fullUpdate": "完整套件更新",
    "about.components": "内置组件",
    "about.unknownVersion": "版本未知",
    "about.ready": "已就绪",
    "about.themeTitle": "外观",
    "about.themeHint": "统一应用于入口和所有模拟器窗口；也可跟随系统。",
    "about.themeAria": "界面外观",
    "about.copyright": "由 Karl-Dai 维护。各模拟器的工程与配置保持独立。",
    "platform.fallback": "桌面版",
  },
  en: {
    "nav.all": "All Simulators",
    "nav.favorites": "Favorites",
    "nav.main": "Main navigation",
    "theme.toggle": "Appearance: {label} (click to cycle)",
    "theme.aria": "Switch appearance, current {label}",
    "theme.system": "System",
    "theme.light": "Light",
    "theme.dark": "Dark",
    "readiness.checking": "Checking suite",
    "readiness.ready": "{ready} / {total} ready",
    "readiness.title": "Re-check bundled suite",
    "lang.aria": "Switch language, current {label}",
    "lang.auto": "System",
    "about.entry": "Settings & About",
    "settings.title": "Settings",
    "settings.language": "Language",
    "workspace.independent": "Simulators open in separate windows",
    "recent.label": "Recent",
    "recent.section": "Recently used",
    "grid.section": "Protocol simulators",
    "role.open": "Open",
    "role.repair": "Repair needed",
    "role.openAria": "Open {protocol} {role}",
    "role.favAria.on": "Star {protocol} {role}",
    "role.favAria.off": "Unstar {protocol} {role}",
    "roles.loading": "Loading bundled apps…",
    "roles.missing": "Suite manifest unavailable",
    "purpose.iec104": "Telecontrol & power dispatch testing",
    "purpose.modbus": "PLC, meters & register testing",
    "purpose.pmu": "Synchrophasor streaming",
    "purpose.opcua": "Address space, subscriptions & tags",
    "empty.title": "Keep frequent tools close",
    "empty.body": "Star a role to pin it here.",
    "empty.browse": "Browse all simulators",
    "toast.launched": "Launch request sent to {protocol} {role}",
    "toast.close": "Dismiss notice",
    "error.title": "Action unavailable",
    "error.help":
      "If bundled files are missing or corrupted, re-run the full installer to repair.",
    "error.dismiss": "OK",
    "error.notReady": "Bundled program unavailable; repair the suite.",
    "about.title": "About Simulator All-in-One",
    "about.suite": "Industrial protocol simulator suite",
    "about.protocols": "{n} protocols · {m} roles",
    "about.fullUpdate": "Full-suite updates",
    "about.components": "Bundled components",
    "about.unknownVersion": "Unknown version",
    "about.ready": "Ready",
    "about.themeTitle": "Appearance",
    "about.themeHint": "Applies to the launcher and all simulator windows, including system appearance.",
    "about.themeAria": "Appearance",
    "about.copyright":
      "Maintained by Karl-Dai. Each simulator keeps its own workspace and configuration.",
    "platform.fallback": "Desktop",
  },
};

const roleNames: Record<string, Record<Locale, string>> = {
  主站: { "zh-CN": "主站", en: "Master" },
  子站: { "zh-CN": "子站", en: "Slave" },
  从站: { "zh-CN": "从站", en: "Slave" },
  客户端: { "zh-CN": "客户端", en: "Client" },
  服务端: { "zh-CN": "服务端", en: "Server" },
};

export function t(
  key: string,
  params?: Record<string, string | number>,
): string {
  let text = messages[locale.value][key] ?? messages.en[key] ?? key;
  if (params) {
    for (const [name, value] of Object.entries(params)) {
      text = text.replaceAll(`{${name}}`, String(value));
    }
  }
  return text;
}

export function roleLabel(name: string): string {
  return roleNames[name]?.[locale.value] ?? name;
}

export function resolveLocale(language: Language): Locale {
  if (language !== "auto") return language;
  return navigator.language?.toLowerCase().startsWith("zh") ? "zh-CN" : "en";
}
