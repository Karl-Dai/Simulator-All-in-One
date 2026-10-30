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
  Info,
  Settings2,
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
import { locale, resolveLocale, roleLabel, t } from "./i18n";
import { PRODUCT_NAME, PRODUCT_SHORT_NAME, PRODUCT_REPOSITORY, PRODUCT_SUMMARY_EN, PRODUCT_SUMMARY_ZH } from "./product";
import type { Language, Preferences, RoleState, Snapshot } from "./types";

const protocols = [
  {
    id: "iec104",
    title: "IEC 60870-5-104",
    icon: Network,
  },
  {
    id: "modbus",
    title: "Modbus",
    icon: Cable,
  },
  {
    id: "pmu",
    title: "PMU",
    icon: Activity,
  },
  {
    id: "opcua",
    title: "OPC UA",
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

const roleTotal = computed(() => snapshot.value?.roles.length || 8);
const readyCount = computed(
  () => snapshot.value?.roles.filter((role) => role.ready).length ?? 0,
);
const favoriteCount = computed(
  () => snapshot.value?.preferences.favorites.length ?? 0,
);
const platformName = computed(
  () =>
    ({
      darwin: "macOS",
      windows: "Windows",
      linux: "Linux",
    })[snapshot.value?.platform ?? ""] ?? t("platform.fallback"),
);
const theme = computed(() => snapshot.value?.preferences.theme ?? "system");
const language = computed(
  () => snapshot.value?.preferences.language ?? "en",
);
const languageBadge = computed(() =>
  language.value === "auto" ? "Auto" : language.value === "en" ? "EN" : "中",
);
const languageLabel = computed(() =>
  language.value === "auto"
    ? t("lang.auto")
    : language.value === "zh-CN"
      ? "中文"
      : "English",
);
const visibleProtocols = computed(() =>
  protocols.filter(
    (protocol) =>
      page.value !== "favorites" ||
      roles(protocol.id).some((role) => isFavorite(role.id)),
  ),
);
function roles(protocol: string): RoleState[] {
  return (
    snapshot.value?.roles.filter((role) => role.protocol === protocol) ?? []
  );
}
function protocolTitle(protocol: string) {
  return protocols.find((item) => item.id === protocol)?.title ?? protocol;
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
    error.value = role.error || t("error.notReady");
    return;
  }
  busy.value = new Set([...busy.value, role.id]);
  try {
    const result = await backend().Launch(role.id);
    notify(
      result.warning ||
        t("toast.launched", {
          protocol: protocolTitle(role.protocol),
          role: roleLabel(role.name),
        }),
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
async function cycleLanguage() {
  const order: Language[] = ["auto", "zh-CN", "en"];
  const next = order[(order.indexOf(language.value) + 1) % order.length];
  try {
    const prefs = await backend().SetLanguage(next);
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
watch(
  locale,
  (value) => {
    document.documentElement.lang = value;
  },
  { immediate: true },
);
watch(language, (value) => setLocale(value), { immediate: true });
function setLocale(value: Language) {
  locale.value = resolveLocale(value);
}
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
    <header class="topbar">
      <div class="brand" :title="`${PRODUCT_NAME} (${PRODUCT_SHORT_NAME})`"><Boxes :size="20" aria-hidden="true" /><strong>{{ PRODUCT_SHORT_NAME }}</strong></div>
      <nav :aria-label="t('nav.main')">
        <button :class="{ selected: page === 'all' }" @click="page = 'all'">
          {{ t("nav.all") }}<span class="nav-count">{{ roleTotal }}</span>
        </button>
        <button
          :class="{ selected: page === 'favorites' }"
          @click="page = 'favorites'"
        >
          {{ t("nav.favorites") }}
          <Star
            v-if="favoriteCount"
            class="nav-star"
            :size="11"
            aria-hidden="true"
          />
        </button>
      </nav>
      <div class="topbar-right">
        <button
          class="icon-button about-link"
          :class="{ selected: page === 'about' }"
          :aria-label="t('about.entry')"
          :title="t('about.entry')"
          @click="page = 'about'"
        >
          <Settings2 :size="18" aria-hidden="true" />
        </button>
      </div>
    </header>

    <main>
      <div class="workspace" v-if="page !== 'about'">
        <div
          v-if="page === 'favorites' && !favoriteCount && !loading"
          class="empty-state"
        >
          <Star :size="28" aria-hidden="true" />
          <h2>{{ t("empty.title") }}</h2>
          <p>{{ t("empty.body") }}</p>
          <button class="ghost-button" @click="page = 'all'">
            {{ t("empty.browse") }} <ArrowUpRight :size="14" aria-hidden="true" />
          </button>
        </div>

        <section class="protocol-grid" :aria-label="t('grid.section')">
          <article
            v-for="protocol in visibleProtocols"
            :key="protocol.id"
            class="protocol-card"
            :data-protocol="protocol.id"
          >
            <div class="card-head">
              <span class="protocol-symbol">
                <component
                  :is="protocol.icon"
                  :size="19"
                  :stroke-width="1.7"
                  aria-hidden="true"
                />
              </span>
              <h2>{{ protocol.title }}</h2>
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
                    :aria-label="
                      t('role.openAria', {
                        protocol: protocol.title,
                        role: roleLabel(role.name),
                      })
                    "
                    :title="role.product"
                    :disabled="busy.has(role.id)"
                    :class="{ unavailable: !role.ready }"
                    @click="launch(role)"
                  >
                    <span class="role-identity"><strong>{{ roleLabel(role.name) }}</strong></span
                    ><span class="launch-action"
                      ><LoaderCircle
                        v-if="busy.has(role.id)"
                        class="spin"
                        :size="15"
                        aria-hidden="true" /><template v-else
                        >{{
                          role.ready ? "" : t("role.repair")
                        }}<ArrowUpRight
                          :size="15"
                          aria-hidden="true" /></template
                    ></span>
                  </button>
                  <button
                    class="favorite-button"
                    :aria-label="
                      t(isFavorite(role.id) ? 'role.favAria.off' : 'role.favAria.on', {
                        protocol: protocol.title,
                        role: roleLabel(role.name),
                      })
                    "
                    :aria-pressed="isFavorite(role.id)"
                    @click="toggleFavorite(role)"
                  >
                    <Star
                      :size="15"
                      :fill="isFavorite(role.id) ? 'currentColor' : 'none'"
                      aria-hidden="true"
                    />
                  </button>
                </div>
              </template>
              <div v-else class="roles-loading">
                {{ loading ? t("roles.loading") : t("roles.missing") }}
              </div>
            </div>
          </article>
        </section>
      </div>

      <div v-else class="workspace about-page">
        <h1>{{ t("about.title") }}</h1>
        <p class="about-description"><span lang="en">{{ PRODUCT_SUMMARY_EN }}</span><span lang="zh-CN">{{ PRODUCT_SUMMARY_ZH }}</span></p>
        <p class="about-meta">
          {{ t("about.suite") }} · v{{ snapshot?.version ?? "0.1.0" }} ·
          {{ platformName }} / {{ snapshot?.architecture ?? "—" }} ·
          {{
            t("about.protocols", {
              n: protocols.length,
              m: roleTotal,
            })
          }}
          · {{ t("about.fullUpdate") }}
        </p>
        <h2 class="section-title">{{ t("about.components") }}</h2>
        <div class="component-table">
          <div v-for="role in snapshot?.roles ?? []" :key="role.id">
            <span>{{ role.product }}</span
            ><span>{{ role.version || t("about.unknownVersion") }}</span
            ><span :class="{ 'component-error': !role.ready }">{{
              role.ready ? t("about.ready") : t("role.repair")
            }}</span>
          </div>
        </div>
        <div class="theme-setting">
          <div><strong>{{ t('settings.language') }}</strong></div>
          <button class="lang-button ghost-button" :aria-label="t('lang.aria', { label: languageLabel })" @click="cycleLanguage">{{ languageLabel }}<span>{{ languageBadge }}</span></button>
        </div>
        <div class="theme-setting">
          <div>
            <strong>{{ t("about.themeTitle") }}</strong>
            <p>{{ t("about.themeHint") }}</p>
          </div>
          <div class="theme-options" :aria-label="t('about.themeAria')">
            <button
              v-for="option in [
                { id: 'system', label: t('theme.system'), icon: Monitor },
                { id: 'light', label: t('theme.light'), icon: Sun },
                { id: 'dark', label: t('theme.dark'), icon: Moon },
              ] as const"
              :key="option.id"
              :aria-pressed="theme === option.id"
              :class="{ active: theme === option.id }"
              @click="changeTheme(option.id as Preferences['theme'])"
            >
              <component :is="option.icon" :size="14" aria-hidden="true" />{{
                option.label
              }}
            </button>
          </div>
        </div>
        <p class="about-copyright">{{ t("about.copyright") }} <a :href="PRODUCT_REPOSITORY" target="_blank" rel="noopener noreferrer">{{ PRODUCT_SHORT_NAME }} · GitHub</a></p>
      </div>
    </main>
    <footer class="workspace-footer">
      <span>{{ t('workspace.independent') }}</span>
      <button class="readiness" :class="{ incomplete: !loading && readyCount < roleTotal }" :disabled="loading" :title="t('readiness.title')" @click="refresh">
        <LoaderCircle v-if="loading" class="spin" :size="13" aria-hidden="true" />
        <RefreshCw v-else :size="13" aria-hidden="true" />
        {{ loading ? t('readiness.checking') : t('readiness.ready', {ready: readyCount, total: roleTotal}) }}
      </button>
    </footer>
    <div v-if="notice" class="toast" role="status">
      <Check :size="16" aria-hidden="true" />{{ notice
      }}<button :aria-label="t('toast.close')" @click="notice = ''">
        <X :size="14" aria-hidden="true" />
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
        <div class="error-icon"><Info :size="22" aria-hidden="true" /></div>
        <h2 id="error-title">{{ t("error.title") }}</h2>
        <p>{{ error }}</p>
        <p class="error-help">{{ t("error.help") }}</p>
        <button ref="errorButton" @click="error = ''">
          {{ t("error.dismiss") }}
        </button>
      </section>
    </div>
  </div>
</template>
