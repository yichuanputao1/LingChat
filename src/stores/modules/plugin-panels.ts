/**
 * 插件内嵌面板 store（主聊天页右侧侧栏）。
 *
 * 只有界面偏好（开合 / 宽度 / 当前面板）进 localStorage；面板列表与加载错误
 * 都是运行时状态，持久化插件每次 mutation 全量写入，必须排除。
 */
import { defineStore } from "pinia";
import { listPlugins } from "@/api/services/plugins";

/** 侧栏宽度限制：默认 380，最小 300，最大取 min(720, 55vw) 由视图侧钳制。 */
export const PLUGIN_PANEL_DEFAULT_WIDTH = 380;
export const PLUGIN_PANEL_MIN_WIDTH = 300;
export const PLUGIN_PANEL_MAX_WIDTH = 720;

/** 一个可打开的面板入口（插件 + 面板声明）。 */
export interface PluginPanelEntry {
  /** `${pluginId}/${panelId}`，用于切标签与持久化。 */
  key: string;
  pluginId: string;
  pluginName: string;
  panelId: string;
  title: string;
}

export interface PluginPanelStoreState {
  /** 界面偏好 */
  open: boolean;
  width: number;
  activeKey: string | null;

  /** 运行时状态（不持久化） */
  panels: PluginPanelEntry[];
  loading: boolean;
  error: string;
}

export const usePluginPanelStore = defineStore("plugin-panels", {
  state: (): PluginPanelStoreState => ({
    open: false,
    width: PLUGIN_PANEL_DEFAULT_WIDTH,
    activeKey: null,

    panels: [],
    loading: false,
    error: "",
  }),

  persist: {
    key: "lingchat-plugin-panels",
    exclude: ["panels", "loading", "error"],
  },

  getters: {
    activePanel(state): PluginPanelEntry | null {
      return state.panels.find((panel) => panel.key === state.activeKey) ?? null;
    },
  },

  actions: {
    /** 重新拉取插件列表，只保留已启用且声明了面板的插件。 */
    async refresh() {
      this.loading = true;
      this.error = "";
      try {
        const plugins = await listPlugins();
        this.panels = plugins
          .filter((plugin) => plugin.enabled && plugin.panels.length > 0)
          .flatMap((plugin) =>
            plugin.panels.map((panel) => ({
              key: `${plugin.id}/${panel.id}`,
              pluginId: plugin.id,
              pluginName: plugin.name,
              panelId: panel.id,
              title: panel.title,
            })),
          );
        if (!this.panels.some((panel) => panel.key === this.activeKey)) {
          this.activeKey = this.panels[0]?.key ?? null;
        }
        if (this.panels.length === 0) this.open = false;
      } catch (error) {
        this.error = String(error);
        this.panels = [];
        this.activeKey = null;
        this.open = false;
      } finally {
        this.loading = false;
      }
    },
  },
});
