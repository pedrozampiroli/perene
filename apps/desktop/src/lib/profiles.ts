// Perfis de ferramenta: shell / claude / codex / opencode, com ícone, cor e o
// comando de spawn (incluindo as flags YOLO). O comando só roda no PRIMEIRO
// spawn de um pane; reattaches ao daemon o ignoram (o PTY já existe).

import type { Pane, Settings } from "./types";

export interface ToolProfile {
  id: string;
  label: string;
  color: string;
}

// Cores de marca iguais às da v1 (IconProvider.brandColor). O ícone em si é o
// SVG real da marca, renderizado pelo componente ToolIcon.
export const PROFILES: ToolProfile[] = [
  { id: "shell", label: "Shell", color: "#9aa0a6" },
  { id: "claude", label: "Claude", color: "#d97557" },
  { id: "codex", label: "Codex", color: "#0fa37f" },
  { id: "opencode", label: "OpenCode", color: "#5c8cfa" },
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

  // Fallback gracioso: se o resume falhar (sessão inexistente/cwd mudou), inicia
  // uma sessão nova em vez de largar o usuário num "session not found" + shell.
  // Para o Claude, o fresh mantém o mesmo id (`--session-id`) pra próxima vez o
  // resume funcionar.
  const claudeFresh = `claude --session-id ${id}${yolo}`;
  const codexFresh = `codex${yolo}`;
  const opencodeFresh = `opencode${yolo}`;

  if (pane.resumeExisting) {
    // Aberto do histórico: retoma a sessão exata.
    switch (p) {
      case "claude":
        return `claude --resume ${id}${yolo} || ${claudeFresh}`;
      case "codex":
        return `codex resume ${id}${yolo} || ${codexFresh}`;
      case "opencode":
        return `opencode --session ${id}${yolo} || ${opencodeFresh}`;
      default:
        return null;
    }
  }

  if (isFresh) {
    switch (p) {
      case "claude":
        return claudeFresh;
      case "codex":
        return codexFresh;
      case "opencode":
        return opencodeFresh;
      default:
        return null;
    }
  }

  // Restauração pós-reboot: retoma a conversa mais recente do diretório.
  switch (p) {
    case "claude":
      return `claude --resume ${id}${yolo} || ${claudeFresh}`;
    case "codex":
      return `codex resume --last${yolo} || ${codexFresh}`;
    case "opencode":
      return `opencode --continue${yolo} || ${opencodeFresh}`;
    default:
      return null;
  }
}
