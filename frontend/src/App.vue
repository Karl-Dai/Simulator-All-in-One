<script setup lang="ts">
import {
  computed,
  nextTick,
  onBeforeUnmount,
  onMounted,
  ref,
  watch,
} from "vue";
import {
  Activity,
  ArrowUpRight,
  Boxes,
  Cable,
  Check,
  ChevronRight,
  Clock3,
  Cpu,
  Grid2X2,
  Info,
  LoaderCircle,
  Monitor,
  Moon,
  Network,
  RefreshCw,
  Star,
  Sun,
  X,
} from "@lucide/vue";
import { backend } from "./bridge";
import type { Preferences, RoleState, Snapshot } from "./types";

const protocols = [
  {
    id: "iec104",
    title: "IEC 104",
    standard: "IEC 60870-5-104",
    purpose: "远动与电力调度联调",
    detail: "召唤、遥控与点表仿真",
    transport: "TCP / TLS",
    icon: Network,
  },
  {
    id: "modbus",
    title: "Modbus",
    standard: "INDUSTRIAL COMMUNICATION",
    purpose: "PLC、仪表与寄存器联调",
    detail: "线圈、寄存器与动态数据源",
    transport: "TCP / TLS / RTU / ASCII",
    icon: Cable,
  },
  {
    id: "pmu",
    title: "PMU",
    standard: "SYNCHROPHASOR",
    purpose: "同步相量数据收发",
    detail: "配置握手与实时相量数据流",
    transport: "V2 / V3",
    icon: Activity,
  },
  {
    id: "opcua",
    title: "OPC UA",
    standard: "OPC UNIFIED ARCHITECTURE",
    purpose: "节点浏览、订阅与变量仿真",
    detail: "地址空间、读写与安全连接",
    transport: "OPC UA / TCP",
    icon: Boxes,
  },
];
const snapshot = ref<Snapshot | null>(null);
const loading = ref(true);
const page = ref<"all" | "favorites" | "about">("all");
const busy = ref(new Set<string>());
const error = ref("");
const notice = ref("");
const errorButton = ref<HTMLButtonElement | null>(null);
let previousFocus: HTMLElement | null = null;
let noticeTimer: ReturnType<typeof setTimeout> | undefined;

const readyCount = computed(
  () => snapshot.value?.roles.filter((role) => role.ready).length ?? 0,
);
const favoriteCount = computed(
  () => snapshot.value?.preferences.favorites.length ?? 0,
);
const platformName = computed(
  () =>
    ({ darwin: "macOS", windows: "Windows", linux: "Linux" })[
      snapshot.value?.platform ?? ""
    ] ?? "桌面版",
);
const theme = computed(() => snapshot.value?.preferences.theme ?? "system");
const visibleProtocols = computed(() =>
  protocols.filter(
    (protocol) =>
      page.value !== "favorites" ||
      roles(protocol.id).some((role) => isFavorite(role.id)),
  ),
);
const recent = computed(() =>
  (snapshot.value?.roles ?? [])
    .filter((role) => snapshot.value?.preferences.recent[role.id])
    .sort((a, b) =>
      (snapshot.value?.preferences.recent[b.id] ?? "").localeCompare(
        snapshot.value?.preferences.recent[a.id] ?? "",
      ),
    )
    .slice(0, 4),
);
function roles(protocol: string): RoleState[] {
  return (
    snapshot.value?.roles.filter((role) => role.protocol === protocol) ?? []
  );
}
function isFavorite(id: string) {
  return snapshot.value?.preferences.favorites.includes(id) ?? false;
}
function notify(text: string) {
  notice.value = text;
  clearTimeout(noticeTimer);
  noticeTimer = setTimeout(() => {
    notice.value = "";
  }, 4200);
}
function message(value: unknown) {
  return value instanceof Error ? value.message : String(value);
}

async function refresh() {
  loading.value = true;
  try {
    snapshot.value = await backend().Snapshot();
    if (snapshot.value.warning) notify(snapshot.value.warning);
  } catch (cause) {
    error.value = message(cause);
  } finally {
    loading.value = false;
  }
}
async function launch(role: RoleState) {
  if (busy.value.has(role.id)) return;
  if (!role.ready) {
    error.value = role.error || "内置程序不可用，请修复完整套件。";
    return;
  }
  busy.value = new Set([...busy.value, role.id]);
  try {
    const result = await backend().Launch(role.id);
    notify(
      result.warning ||
        `已向 ${protocols.find((p) => p.id === role.protocol)?.title} ${role.name} 提交启动请求`,
    );
    if (snapshot.value)
      snapshot.value.preferences.recent[role.id] = new Date().toISOString();
  } catch (cause) {
    error.value = message(cause);
  } finally {
    const next = new Set(busy.value);
    next.delete(role.id);
    busy.value = next;
  }
}
async function toggleFavorite(role: RoleState) {
  try {
    const prefs = await backend().SetFavorite(role.id, !isFavorite(role.id));
    if (snapshot.value) snapshot.value.preferences = prefs;
  } catch (cause) {
    error.value = message(cause);
  }
}
async function changeTheme(value: Preferences["theme"]) {
  try {
    const prefs = await backend().SetTheme(value);
    if (snapshot.value) snapshot.value.preferences = prefs;
  } catch (cause) {
    error.value = message(cause);
  }
}
watch(
  theme,
  (value) => {
    document.documentElement.dataset.theme = value;
  },
  { immediate: true },
);
watch(error, async (value) => {
  if (value) {
    previousFocus = document.activeElement as HTMLElement | null;
    await nextTick();
    errorButton.value?.focus();
  } else if (previousFocus?.isConnected) previousFocus.focus();
});
function trapErrorFocus(event: KeyboardEvent) {
  if (event.key === "Tab") {
    event.preventDefault();
    errorButton.value?.focus();
  }
}
onBeforeUnmount(() => clearTimeout(noticeTimer));
onMounted(refresh);
</script>

<template>
  <div class="shell">
    <aside class="sidebar" aria-label="主导航">
      <div class="brand">
        <div class="brand-mark" aria-hidden="true"><i></i><i></i><i></i></div>
        <div>
          <strong>SimLab<span>Desktop</span></strong
          ><small>工业协议模拟器套件</small>
        </div>
      </div>
      <p class="nav-label">工作空间</p>
      <nav>
        <button :class="{ selected: page === 'all' }" @click="page = 'all'">
          <Grid2X2 :size="18" aria-hidden="true" />全部模拟器<span
            class="nav-count"
            >8</span
          >
        </button>
        <button
          :class="{ selected: page === 'favorites' }"
          @click="page = 'favorites'"
        >
          <Star :size="18" aria-hidden="true" />常用模拟器<span
            v-if="favoriteCount"
            class="nav-count"
            >{{ favoriteCount }}</span
          >
        </button>
      </nav>
      <div class="sidebar-bottom">
        <div class="offline-note">
          <span class="status-dot" aria-hidden="true"></span>随包内置 · 离线可用
        </div>
        <button
          class="about-link"
          :class="{ selected: page === 'about' }"
          @click="page = 'about'"
        >
          <Info :size="17" aria-hidden="true" />关于 SimLab<ChevronRight
            :size="15"
            aria-hidden="true"
          />
        </button>
      </div>
    </aside>

    <main>
      <header class="topbar">
        <span class="breadcrumb"
          >工作空间 <ChevronRight :size="13" aria-hidden="true" />{{
            page === "all"
              ? "全部模拟器"
              : page === "favorites"
                ? "常用模拟器"
                : "关于 SimLab"
          }}</span
        >
        <div class="topbar-right">
          <span class="platform"
            ><Monitor :size="14" aria-hidden="true" />{{ platformName }}</span
          ><span class="top-divider"></span
          ><span class="version">v{{ snapshot?.version ?? "0.1.0" }}</span>
        </div>
      </header>

      <div class="workspace" v-if="page !== 'about'">
        <div class="page-heading">
          <div>
            <p class="eyebrow">PROTOCOL WORKSPACE</p>
            <h1>
              {{
                page === "all" ? "选择模拟器，开始联调。" : "你的常用模拟器。"
              }}
            </h1>
            <p class="intro">
              {{
                page === "all"
                  ? "四种协议，八个角色。需要的工具都在这里。"
                  : "收藏常用角色，下次直接打开。"
              }}
            </p>
          </div>
          <div
            class="readiness"
            :class="{ incomplete: !loading && readyCount < 8 }"
          >
            <LoaderCircle
              v-if="loading"
              class="spin"
              :size="16"
              aria-hidden="true"
            /><Check
              v-else-if="readyCount === 8"
              :size="16"
              aria-hidden="true"
            /><RefreshCw v-else :size="15" aria-hidden="true" /><span>{{
              loading ? "正在检查套件" : `${readyCount} / 8 已就绪`
            }}</span>
          </div>
        </div>

        <section
          v-if="recent.length && page === 'all'"
          class="recent-section"
          aria-label="最近使用"
        >
          <span class="recent-label"
            ><Clock3 :size="15" aria-hidden="true" />最近使用</span
          ><button
            v-for="role in recent"
            :key="role.id"
            @click="launch(role)"
            :disabled="busy.has(role.id)"
          >
            {{ protocols.find((p) => p.id === role.protocol)?.title
            }}<span>{{ role.name }}</span
            ><ArrowUpRight :size="13" aria-hidden="true" />
          </button>
        </section>

        <div
          v-if="page === 'favorites' && !favoriteCount && !loading"
          class="empty-state"
        >
          <Star :size="30" aria-hidden="true" />
          <h2>把常用工具放在手边</h2>
          <p>在模拟器角色旁点击星标，即可收藏。</p>
          <button @click="page = 'all'">
            浏览全部模拟器 <ArrowUpRight :size="15" aria-hidden="true" />
          </button>
        </div>

        <section class="protocol-grid" aria-label="协议模拟器">
          <article
            v-for="protocol in visibleProtocols"
            :key="protocol.id"
            class="protocol-card"
            :data-protocol="protocol.id"
          >
            <div class="card-header">
              <div class="protocol-symbol">
                <component
                  :is="protocol.icon"
                  :size="25"
                  :stroke-width="1.6"
                  aria-hidden="true"
                />
              </div>
              <span class="transport">{{ protocol.transport }}</span>
            </div>
            <div class="protocol-identity">
              <p class="standard">{{ protocol.standard }}</p>
              <h2>{{ protocol.title }}</h2>
              <p class="purpose">{{ protocol.purpose }}</p>
              <p class="detail">{{ protocol.detail }}</p>
            </div>
            <div class="role-list">
              <template v-if="roles(protocol.id).length">
                <div
                  v-for="role in roles(protocol.id)"
                  :key="role.id"
                  v-show="page !== 'favorites' || isFavorite(role.id)"
                  class="role-row"
                >
                  <button
                    class="launch-button"
                    :aria-label="`打开 ${protocol.title} ${role.name}`"
                    :disabled="busy.has(role.id)"
                    :class="{ unavailable: !role.ready }"
                    @click="launch(role)"
                  >
                    <span
                      ><strong>{{ role.name }}</strong
                      ><small>{{ role.product }}</small></span
                    ><span class="launch-action"
                      ><LoaderCircle
                        v-if="busy.has(role.id)"
                        class="spin"
                        :size="17"
                        aria-hidden="true" /><template v-else
                        >{{ role.ready ? "打开" : "需要修复"
                        }}<ArrowUpRight
                          :size="17"
                          aria-hidden="true" /></template
                    ></span>
                  </button>
                  <button
                    class="favorite-button"
                    :aria-label="`${isFavorite(role.id) ? '取消收藏' : '收藏'} ${protocol.title} ${role.name}`"
                    :aria-pressed="isFavorite(role.id)"
                    @click="toggleFavorite(role)"
                  >
                    <Star
                      :size="16"
                      :fill="isFavorite(role.id) ? 'currentColor' : 'none'"
                      aria-hidden="true"
                    />
                  </button>
                </div>
              </template>
              <div v-else class="roles-loading">
                {{ loading ? "正在读取内置应用…" : "未能读取套件清单" }}
              </div>
            </div>
          </article>
        </section>
        <footer class="workspace-footer">
          <span
            ><Cpu
              :size="14"
              aria-hidden="true"
            />每个模拟器独立运行，关闭入口不影响联调。</span
          ><button @click="refresh" :disabled="loading">
            <RefreshCw
              :size="13"
              :class="{ spin: loading }"
              aria-hidden="true"
            />重新检查
          </button>
        </footer>
      </div>

      <div v-else class="workspace about-page">
        <p class="eyebrow">ABOUT SIMLAB</p>
        <h1>一个套件，完整的联调工具。</h1>
        <p class="intro">
          SimLab Desktop · v{{ snapshot?.version ?? "0.1.0" }}
        </p>
        <div class="about-summary">
          <div>
            <span>运行平台</span
            ><strong
              >{{ platformName }} / {{ snapshot?.architecture ?? "—" }}</strong
            >
          </div>
          <div><span>内置模拟器</span><strong>4 种协议 · 8 个角色</strong></div>
          <div><span>更新方式</span><strong>完整套件更新</strong></div>
        </div>
        <h2 class="section-title">内置组件</h2>
        <div class="component-table">
          <div v-for="role in snapshot?.roles ?? []" :key="role.id">
            <span>{{ role.product }}</span
            ><span>{{ role.version || "版本未知" }}</span
            ><span :class="{ 'component-error': !role.ready }">{{
              role.ready ? "已就绪" : "需要修复"
            }}</span>
          </div>
        </div>
        <div class="theme-setting">
          <div>
            <strong>外观</strong>
            <p>选择浅色、深色，或跟随系统。</p>
          </div>
          <div class="theme-options" aria-label="界面外观">
            <button
              v-for="option in [
                { id: 'system', label: '跟随系统', icon: Monitor },
                { id: 'light', label: '浅色', icon: Sun },
                { id: 'dark', label: '深色', icon: Moon },
              ] as const"
              :key="option.id"
              :aria-pressed="theme === option.id"
              :class="{ active: theme === option.id }"
              @click="changeTheme(option.id)"
            >
              <component :is="option.icon" :size="15" aria-hidden="true" />{{
                option.label
              }}
            </button>
          </div>
        </div>
        <p class="about-copyright">
          由 Karl-Dai 维护。各模拟器的工程与配置保持独立。
        </p>
      </div>
    </main>
    <div v-if="notice" class="toast" role="status">
      <Check :size="17" aria-hidden="true" />{{ notice
      }}<button aria-label="关闭提示" @click="notice = ''">
        <X :size="15" aria-hidden="true" />
      </button>
    </div>
    <div
      v-if="error"
      class="error-backdrop"
      @keydown.esc="error = ''"
      @keydown="trapErrorFocus"
    >
      <section
        class="error-dialog"
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="error-title"
      >
        <div class="error-icon"><Info :size="24" aria-hidden="true" /></div>
        <h2 id="error-title">暂时无法完成操作</h2>
        <p>{{ error }}</p>
        <p class="error-help">
          如果套件文件缺失或损坏，请重新运行完整安装包进行修复。
        </p>
        <button ref="errorButton" @click="error = ''">知道了</button>
      </section>
    </div>
  </div>
</template>
