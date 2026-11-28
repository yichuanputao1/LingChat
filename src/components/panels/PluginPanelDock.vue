<template>
  <!-- 关闭态：贴右边缘的竖向把手。导演侧栏占着 50%，这里错开一截 -->
  <button
    v-if="store.panels.length"
    class="plugin-panel-tab plugin-panel-handle"
    :class="{ 'plugin-panel-tab-hidden': store.open }"
    :title="t('views.pluginPanel.title')"
    @click="openDock"
  >
    <PanelRight :size="16" />
    <span class="plugin-panel-tab-label">{{ t("views.pluginPanel.title") }}</span>
  </button>

  <!-- 展开态：右侧面板，从右边缘滑入 / 滑出 -->
  <Transition name="plugin-panel-slide">
    <aside v-if="store.open && store.panels.length" class="plugin-panel" :style="panelStyle">
      <!-- 面板左边缘的竖标签：点它收回面板；窄屏抽屉没有多余空间 -->
      <button
        v-if="!narrow"
        class="plugin-panel-tab plugin-panel-tab-panel"
        :title="t('views.pluginPanel.title')"
        @click="store.open = false"
      >
        <PanelRight :size="16" />
        <span class="plugin-panel-tab-label">{{ t("views.pluginPanel.title") }}</span>
      </button>

      <!-- 左边缘拖拽手柄（窄屏退化为抽屉，不显示） -->
      <div v-if="!narrow" class="plugin-panel-grip" @pointerdown="onGripDown"></div>

      <header class="flex shrink-0 items-center gap-2 px-4 pt-3 pb-2">
        <PanelRight :size="13" class="shrink-0 text-white/60" />
        <span class="min-w-0 truncate text-[0.86rem] font-semibold text-white/90">
          {{ activePanel?.title ?? t("views.pluginPanel.title") }}
        </span>
        <span v-if="activePanel" class="min-w-0 truncate text-[0.64rem] text-white/40">
          {{ activePanel.pluginName }}
        </span>
        <button
          class="ml-auto shrink-0 rounded-md p-1 text-white/40 transition-colors hover:bg-white/10 hover:text-white/85"
          :title="t('views.pluginPanel.close')"
          @click="store.open = false"
        >
          <X :size="14" />
        </button>
      </header>

      <!-- 多个面板时在标题下排一行标签 -->
      <nav v-if="store.panels.length > 1" class="flex shrink-0 gap-1 overflow-x-auto px-3 pb-2">
        <button
          v-for="panel in store.panels"
          :key="panel.key"
          class="inline-flex shrink-0 items-center gap-1.5 rounded-lg px-2 py-1.5 text-[0.74rem] transition-colors duration-200"
          :class="
            panel.key === store.activeKey
              ? 'bg-white/12 text-white/90'
              : 'text-white/50 hover:bg-white/6 hover:text-white/75'
          "
          @click="store.activeKey = panel.key"
        >
          {{ panel.title }}
        </button>
      </nav>

      <div class="relative min-h-0 flex-1">
        <div
          v-if="store.error || loadError"
          class="flex h-full items-center justify-center px-6 text-center text-[0.76rem] leading-relaxed text-red-300/80"
        >
          {{ t("views.pluginPanel.failed", { error: store.error || loadError }) }}
        </div>
        <template v-else-if="url">
          <iframe
            ref="frameRef"
            :key="store.activeKey ?? 'none'"
            :src="url"
            class="h-full w-full border-0 bg-transparent"
            sandbox="allow-scripts allow-forms allow-popups allow-downloads"
            referrerpolicy="no-referrer"
            @load="postState"
          ></iframe>
          <div
            v-if="loading"
            class="pointer-events-none absolute inset-0 flex items-center justify-center bg-[rgba(18,18,28,0.6)] text-[0.76rem] text-white/60"
          >
            {{ t("views.pluginPanel.loading") }}
          </div>
        </template>
        <div v-else class="flex h-full items-center justify-center text-[0.76rem] text-white/50">
          {{ t("views.pluginPanel.loading") }}
        </div>
      </div>
    </aside>
  </Transition>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { PanelRight, X } from "@lucide/vue";
import { pluginPanelUrl } from "@/api/services/plugins";
import { buildPanelState } from "@/components/panels/panel-state";
import {
  PLUGIN_PANEL_MAX_WIDTH,
  PLUGIN_PANEL_MIN_WIDTH,
  usePluginPanelStore,
} from "@/stores/modules/plugin-panels";
import { useDirectorStore } from "@/stores/modules/director";
import { useUIStore } from "@/stores/modules/ui/ui";

const { t } = useI18n();
const store = usePluginPanelStore();
const directorStore = useDirectorStore();
const uiStore = useUIStore();

const frameRef = ref<HTMLIFrameElement | null>(null);
const url = ref("");
const loading = ref(false);
const loadError = ref("");
/** 请求序号：快速切标签时丢弃迟到的地址。 */
let loadSeq = 0;

const activePanel = computed(() => store.activePanel);

// ==================== 窄屏 ====================

/** 视口窄于 768px 时退化为抽屉：宽度铺满、不显示拖拽手柄 */
const narrow = ref(false);
const syncNarrow = () => (narrow.value = window.innerWidth < 768);

const panelStyle = computed(() => ({
  width: narrow.value ? "calc(100vw - 16px)" : `${store.width}px`,
}));

// ==================== 打开 / 切换 ====================

async function openDock() {
  // 右侧同一块地方只放一个侧栏：导演侧栏开着就先收起它
  directorStore.open = false;
  store.open = true;
  await store.refresh();
  await loadUrl();
}

async function loadUrl() {
  const panel = store.activePanel;
  if (!panel) {
    url.value = "";
    return;
  }
  const seq = ++loadSeq;
  loading.value = true;
  loadError.value = "";
  try {
    const next = await pluginPanelUrl(panel.pluginId, panel.panelId);
    if (seq === loadSeq) url.value = next;
  } catch (error) {
    if (seq === loadSeq) {
      url.value = "";
      loadError.value = String(error);
    }
  } finally {
    if (seq === loadSeq) loading.value = false;
  }
}

// 打开或切面板时重新取地址；地址带随机端口，不能缓存
watch(
  () => [store.open, store.activeKey] as const,
  ([open]) => {
    if (!open) return;
    url.value = "";
    loadError.value = "";
    void loadUrl();
  },
);

// 导演侧栏打开时让位，避免两个侧栏叠在右边
watch(
  () => directorStore.open,
  (open) => {
    if (open) store.open = false;
  },
);

// 设置页会盖住主界面，先收起面板；关闭设置后不自动弹回，避免打断玩家
watch(
  () => uiStore.showSettings,
  (open) => {
    if (open) store.open = false;
  },
);

// ==================== 拖拽调宽 ====================

/** 从右向左拖出宽度：手柄在左边缘，向左移动变宽 */
function onGripDown(e: PointerEvent) {
  if (narrow.value) return;
  e.preventDefault();
  const startX = e.clientX;
  const startW = store.width;
  const onMove = (ev: PointerEvent) => {
    const maxW = Math.min(PLUGIN_PANEL_MAX_WIDTH, Math.floor(window.innerWidth * 0.55));
    store.width = Math.min(maxW, Math.max(PLUGIN_PANEL_MIN_WIDTH, startW + startX - ev.clientX));
  };
  const onUp = () => {
    window.removeEventListener("pointermove", onMove);
    window.removeEventListener("pointerup", onUp);
  };
  window.addEventListener("pointermove", onMove);
  window.addEventListener("pointerup", onUp);
}

// ==================== 状态推送 ====================

const snapshot = computed(() => buildPanelState());
let postTimer: number | null = null;

function postState() {
  const frame = frameRef.value;
  if (!frame?.contentWindow || !url.value) return;
  frame.contentWindow.postMessage(
    { type: "lingchat:panel:state", state: { ...snapshot.value, at: Date.now() } },
    "*",
  );
}

/** 状态变化可能很密集（流式台词），合并成 300ms 一次。 */
function schedulePost() {
  if (postTimer != null) return;
  postTimer = window.setTimeout(() => {
    postTimer = null;
    postState();
  }, 300);
}

watch(snapshot, schedulePost);

function onMessage(event: MessageEvent) {
  const frame = frameRef.value;
  // 面板是跨源沙箱（origin 为 null），只能靠 source 确认消息来自当前 iframe
  if (!frame || event.source !== frame.contentWindow) return;
  const data = event.data as { type?: string } | null;
  if (data?.type === "lingchat:panel:request-state") postState();
}

onMounted(() => {
  syncNarrow();
  window.addEventListener("resize", syncNarrow);
  window.addEventListener("message", onMessage);
  void store.refresh();
});

onBeforeUnmount(() => {
  window.removeEventListener("resize", syncNarrow);
  window.removeEventListener("message", onMessage);
  if (postTimer != null) window.clearTimeout(postTimer);
});
</script>

<style scoped>
/* 竖向标签：关闭时贴右边缘，展开时贴在面板左边缘 */
.plugin-panel-tab {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
  padding: 12px 6px;
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-right: none;
  border-radius: 12px 0 0 12px;
  background: rgba(18, 18, 28, 0.75);
  color: rgba(255, 255, 255, 0.65);
  backdrop-filter: blur(16px);
  box-shadow: -4px 0 20px rgba(0, 0, 0, 0.35);
  transform: translateY(-50%);
  transition:
    color 0.2s,
    background 0.2s,
    opacity 0.3s cubic-bezier(0.32, 0.72, 0, 1);
}

.plugin-panel-tab:hover {
  background: rgba(28, 28, 40, 0.88);
  color: rgba(255, 255, 255, 0.9);
}

/* 错开导演侧栏的 50%，两个把手都能点到 */
.plugin-panel-handle {
  position: fixed;
  top: 34%;
  right: 0;
  z-index: 998;
}

.plugin-panel-tab-panel {
  position: absolute;
  top: 50%;
  right: 100%;
  z-index: 2;
}

.plugin-panel-tab-hidden {
  opacity: 0;
  pointer-events: none;
}

.plugin-panel-tab-label {
  writing-mode: vertical-rl;
  letter-spacing: 0.3em;
  font-size: 0.72rem;
}

.plugin-panel {
  position: fixed;
  top: calc(74px + var(--safe-area-inset-top));
  right: 0;
  bottom: calc(16px + var(--safe-area-inset-bottom));
  z-index: 998;
  display: flex;
  flex-direction: column;
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-right: none;
  border-radius: 18px 0 0 18px;
  background: rgba(18, 18, 28, 0.78);
  backdrop-filter: blur(20px);
  box-shadow: -8px 0 32px rgba(0, 0, 0, 0.4);
}

.plugin-panel-grip {
  position: absolute;
  top: 50%;
  left: 0;
  width: 6px;
  height: 48px;
  border-radius: 3px;
  background: rgba(255, 255, 255, 0.18);
  transform: translate(-50%, -50%);
  cursor: ew-resize;
}

.plugin-panel-slide-enter-active,
.plugin-panel-slide-leave-active {
  transition:
    transform 0.28s cubic-bezier(0.32, 0.72, 0, 1),
    opacity 0.28s ease;
}

.plugin-panel-slide-enter-from,
.plugin-panel-slide-leave-to {
  opacity: 0;
  transform: translateX(100%);
}
</style>
