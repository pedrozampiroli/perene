// Perfis de ferramenta: shell / claude / codex / opencode, com ícone, cor e o
// comando de spawn (incluindo as flags YOLO). O comando só roda no PRIMEIRO
// spawn de um pane; reattaches ao daemon o ignoram (o PTY já existe).

import type { Pane, Settings } from "./types";

export interface ToolProfile {
  id: string;
  label: string;
  icon: string;
  color: string;
}

export const PROFILES: ToolProfile[] = [
  { id: "shell", label: "Shell", icon: "❯", color: "#9aa0a6" },
  { id: "claude", label: "Claude", icon: "✳", color: "#d97757" },
  { id: "codex", label: "Codex", icon: "◆", color: "#10a37f" },
  { id: "opencode", label: "OpenCode", icon: "◇", color: "#6ea8fe" },
];

export function profile(id: string): ToolProfile {
  return PROFILES.find((p) => p.id === id) ?? PROFILES[0];
}

export function needsSessionId(profileId: string): boolean {
  return profileId === "claude";
}

function yoloFlag(profileId: string, on: boolean): string {
  if (!on) return "";
  switch (profileId) {
    case "claude":
      return " --dangerously-skip-permissions";
    case "codex":
      return " --dangerously-bypass-approvals-and-sandbox";
    case "opencode":
      return " --auto";
    default:
      return "";
  }
}

/**
 * Comando do login shell para um pane. `null` = shell puro.
 *
 * Três modos:
 *  - `fresh`: pane novo → cria sessão (`claude --session-id`, `codex`, `opencode`).
 *  - `reboot`: pane restaurado após o daemon morrer → retoma a última conversa
 *    daquele diretório (`claude --resume <id>`, `codex resume --last`,
 *    `opencode --continue`).
 *  - histórico (`pane.resumeExisting`): retoma uma sessão específica por id
 *    (`claude --resume <id>`, `codex resume <id>`, `opencode --session <id>`).
 *
 * Nunca usa `claude --continue`.
 */
export function buildCommand(pane: Pane, settings: Settings, isFresh: boolean): string | null {
  const p = pane.toolProfileId;
  const id = pane.harnessSessionId ?? "";
  const yolo = yoloFlag(p, settings.yolo);

  if (pane.resumeExisting) {
    // Aberto do histórico: retoma a sessão exata.
    switch (p) {
      case "claude":
        return `claude --resume ${id}${yolo}`;
      case "codex":
        return `codex resume ${id}${yolo}`;
      case "opencode":
        return `opencode --session ${id}${yolo}`;
      default:
        return null;
    }
  }

  if (isFresh) {
    switch (p) {
      case "claude":
        return `claude --session-id ${id}${yolo}`;
      case "codex":
        return `codex${yolo}`;
      case "opencode":
        return `opencode${yolo}`;
      default:
        return null;
    }
  }

  // Restauração pós-reboot: retoma a conversa mais recente do diretório.
  switch (p) {
    case "claude":
      return `claude --resume ${id}${yolo}`;
    case "codex":
      return `codex resume --last${yolo}`;
    case "opencode":
      return `opencode --continue${yolo}`;
    default:
      return null;
  }
}
