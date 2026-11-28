/**
 * 推给插件面板的游戏状态快照。
 *
 * 面板 iframe 是跨源沙箱，拿不到 Pinia 与 Tauri IPC，所有状态都靠宿主
 * 通过 postMessage 主动推送。字段只增不改：面板按 version 判断兼容性。
 */
import { useGameStore } from "@/stores/modules/game";
import type { AffectionVector, NegativeVector } from "@/stores/modules/game/state";
import { useUIStore } from "@/stores/modules/ui/ui";

export const PANEL_STATE_VERSION = 1;

export interface PanelStateSnapshot {
  version: number;
  status: string;
  currentLine: string;
  player: {
    name: string;
    subtitle: string;
    roleId: number | null;
  };
  currentRoleId: number | null;
  roles: Array<{
    id: number;
    name: string;
    subtitle: string;
    emotion: string;
    present: boolean;
    affection: AffectionVector | null;
    negative: NegativeVector | null;
  }>;
  scene: {
    id: string;
    name: string;
    description: string;
    background: string | null;
  } | null;
  background: string;
  presentPic: string;
  music: string;
  effect: string;
  autoMode: boolean;
  script: {
    name: string;
    chapter: string;
    running: boolean;
  } | null;
  recentLines: Array<{
    type: string;
    name: string;
    content: string;
    emotion: string | null;
    senderRoleId: number | null;
  }>;
}

/** 当前游戏状态快照；只读取面板会展示的字段，避免把整份存档推给页面。 */
export function buildPanelState(): PanelStateSnapshot {
  const game = useGameStore();
  const ui = useUIStore();

  return {
    version: PANEL_STATE_VERSION,
    status: game.currentStatus,
    currentLine: game.currentLine,
    player: {
      name: game.userName,
      subtitle: game.userSubtitle,
      roleId: game.playerRoleId,
    },
    currentRoleId: game.currentInteractRoleId,
    roles: Object.values(game.gameRoles).map((role) => ({
      id: role.roleId,
      name: role.roleName,
      subtitle: role.roleSubTitle,
      emotion: role.emotion,
      present: game.presentRoleIds.includes(role.roleId),
      affection: role.affection ?? null,
      negative: role.negative ?? null,
    })),
    scene: game.currentScene
      ? {
          id: game.currentScene.id,
          name: game.currentScene.scene_name,
          description: game.currentScene.scene_description,
          background: game.currentScene.background,
        }
      : null,
    background: ui.currentBackground,
    presentPic: ui.currentPresentPic,
    music: ui.currentBackgroundMusic,
    effect: ui.currentBackgroundEffect,
    autoMode: ui.autoMode,
    script: game.runningScript
      ? {
          name: game.runningScript.scriptName,
          chapter: game.runningScript.currentChapterName,
          running: game.runningScript.isRunning,
        }
      : null,
    recentLines: game.dialogHistory.slice(-8).map((line) => ({
      type: line.type,
      name: line.displayName,
      content: line.content,
      emotion: line.emotion ?? null,
      senderRoleId: line.senderRoleId ?? null,
    })),
  };
}
