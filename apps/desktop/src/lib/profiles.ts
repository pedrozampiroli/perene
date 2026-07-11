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

/**
 * Comando do login shell para um pane. `null` = shell puro.
 * YOLO adiciona a flag de "pular permissões" de cada harness.
 */
export function buildCommand(pane: Pane, settings: Settings): string | null {
  switch (pane.toolProfileId) {
    case "claude": {
      // Nunca `--continue` (colide com múltiplas sessões na mesma pasta).
      const id = pane.harnessSessionId ?? "";
      const yolo = settings.yolo ? " --dangerously-skip-permissions" : "";
      return `claude --session-id ${id}${yolo}`;
    }
    case "codex": {
      const yolo = settings.yolo ? " --dangerously-bypass-approvals-and-sandbox" : "";
      return `codex${yolo}`;
    }
    case "opencode": {
      const yolo = settings.yolo ? " --auto" : "";
      return `opencode${yolo}`;
    }
    default:
      return null; // shell
  }
}
