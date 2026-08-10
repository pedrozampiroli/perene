// Estado das conversas ACP na UI.
//
// O daemon é a fonte da verdade: ele guarda o transcript e o reenvia inteiro no
// attach. Este módulo só *reduz* o stream de eventos a algo desenhável — por
// isso `reset()` antes de atachar: a conversa é reconstruída do zero a cada vez,
// sem risco de duplicar o que já estava na tela.
//
// O `update` chega cru do agente (`session/update` do ACP). Lemos apenas os
// campos que sabemos desenhar e ignoramos o resto: o protocolo evolui, e uma
// variante nova não pode quebrar a tela.

import type {
  AcpCommand,
  AcpEvent,
  AcpImage,
  AcpModels,
  AcpModes,
  AcpOption,
  AcpPermissionOption,
} from "./types";

/** Um pedaço renderizável dentro do cartão de uma ferramenta. */
export type AcpToolPart =
  | { type: "text"; text: string }
  | { type: "diff"; path: string; oldText: string | null; newText: string }
  /** A saída chega à parte, pelo evento `terminal` — aqui fica só a referência. */
  | { type: "terminal"; terminalId: string };

export interface AcpBlock {
  id: number;
  kind: "message" | "thought" | "tool" | "notice";
  /** Quem falou (só em `message`). */
  role?: "agent" | "user";
  text: string;
  /** Só em `tool`. */
  toolCallId?: string;
  toolKind?: string;
  status?: string;
  parts?: AcpToolPart[];
  /** Arquivos que a ferramenta toca. */
  locations?: { path: string; line?: number | null }[];
  /** Só em `notice`. */
  level?: "error" | "info";
}

export interface AcpPending {
  requestId: number;
  title: string;
  options: AcpPermissionOption[];
}

/** Saída de um comando que o Perene rodou a pedido do agente. */
export interface AcpTerminal {
  output: string;
  truncated: boolean;
  exitCode: number | null;
}

export interface AcpConversation {
  /** `session/new` respondeu: já dá para mandar prompt. */
  ready: boolean;
  /** Turno em andamento. */
  busy: boolean;
  blocks: AcpBlock[];
  /** Plano de execução, quando o agente publica um. */
  plan: { text: string; status: string }[];
  /** Pedido de permissão aguardando o usuário (um por vez). */
  permission: AcpPending | null;
  /** Comandos de barra que a sessão aceita. Vem do agente, não é lista nossa. */
  commands: AcpCommand[];
  /** Saídas de comando, por `terminalId`. */
  terminals: Record<string, AcpTerminal>;
  modes: { current: string; available: AcpOption[] };
  models: { current: string; available: AcpOption[] };
  /** Contexto consumido (`used`/`size` em tokens). */
  usage: { used: number; size: number } | null;
  nextId: number;
}

export function emptyConversation(): AcpConversation {
  return {
    ready: false,
    busy: false,
    blocks: [],
    plan: [],
    permission: null,
    commands: [],
    terminals: {},
    modes: { current: "", available: [] },
    models: { current: "", available: [] },
    usage: null,
    nextId: 1,
  };
}

/** Texto de um bloco de conteúdo do ACP (`{type:"text"}`, string solta, lista). */
function contentText(content: unknown): string {
  if (typeof content === "string") return content;
  if (Array.isArray(content)) return content.map(contentText).join("");
  if (content && typeof content === "object") {
    const c = content as Record<string, unknown>;
    if (typeof c.text === "string") return c.text;
  }
  return "";
}

function str(value: unknown, fallback = ""): string {
  return typeof value === "string" ? value : fallback;
}

function list(value: unknown): Record<string, unknown>[] {
  return Array.isArray(value) ? (value.filter(Boolean) as Record<string, unknown>[]) : [];
}

/** Acrescenta texto ao último bloco do mesmo tipo, ou abre um novo. */
function appendText(
  conv: AcpConversation,
  kind: AcpBlock["kind"],
  role: AcpBlock["role"],
  text: string,
) {
  if (!text) return;
  const last = conv.blocks[conv.blocks.length - 1];
  if (last && last.kind === kind && last.role === role) {
    last.text += text;
    return;
  }
  conv.blocks.push({ id: conv.nextId++, kind, role, text });
}

function notice(conv: AcpConversation, level: "error" | "info", text: string) {
  conv.blocks.push({ id: conv.nextId++, kind: "notice", level, text });
}

/** Título legível de uma tool call (o agente nem sempre manda `title`). */
function toolTitle(toolCall: Record<string, unknown>): string {
  const title = str(toolCall.title);
  if (title) return title;
  const raw = toolCall.rawInput;
  if (raw && typeof raw === "object") {
    const r = raw as Record<string, unknown>;
    const guess = str(r.command) || str(r.path) || str(r.file_path);
    if (guess) return guess;
  }
  return str(toolCall.toolCallId, "ferramenta");
}

/** Converte o `content` de uma tool call nos pedaços que sabemos desenhar. */
function toolParts(content: unknown): AcpToolPart[] {
  return list(content)
    .map((item): AcpToolPart | null => {
      switch (item.type) {
        case "diff":
          return {
            type: "diff",
            path: str(item.path),
            oldText: typeof item.oldText === "string" ? item.oldText : null,
            newText: str(item.newText),
          };
        case "terminal":
          return { type: "terminal", terminalId: str(item.terminalId) };
        case "content": {
          const text = contentText(item.content);
          return text ? { type: "text", text } : null;
        }
        default:
          return null; // formato que ainda não desenhamos
      }
    })
    .filter((p): p is AcpToolPart => p !== null);
}

function toolLocations(value: unknown): AcpBlock["locations"] {
  return list(value)
    .map((l) => ({
      path: str(l.path),
      line: typeof l.line === "number" ? l.line : null,
    }))
    .filter((l) => l.path);
}

/** Normaliza modos e modelos: o wire usa `id` num e `modelId` no outro. */
function readModes(modes: AcpModes | null | undefined) {
  return {
    current: str(modes?.currentModeId),
    available: list(modes?.availableModes).map((m) => ({
      id: str(m.id),
      name: str(m.name),
      description: str(m.description) || null,
    })),
  };
}

function readModels(models: AcpModels | null | undefined) {
  return {
    current: str(models?.currentModelId),
    available: list(models?.availableModels).map((m) => ({
      id: str(m.modelId),
      name: str(m.name),
      description: str(m.description) || null,
    })),
  };
}

/** Aplica um evento do daemon à conversa. Muta (o `$state` é reativo em profundidade). */
export function applyAcpEvent(conv: AcpConversation, event: AcpEvent): void {
  switch (event.kind) {
    case "ready":
      conv.ready = true;
      conv.modes = readModes(event.modes);
      conv.models = readModels(event.models);
      return;

    case "terminal":
      conv.terminals[event.terminalId] = {
        output: event.output,
        truncated: event.truncated,
        exitCode: event.exitCode,
      };
      return;

    case "update": {
      const u = event.update as Record<string, unknown>;
      switch (u.sessionUpdate) {
        case "agent_message_chunk":
          appendText(conv, "message", "agent", contentText(u.content));
          return;
        case "user_message_chunk":
          appendText(conv, "message", "user", contentText(u.content));
          return;
        case "agent_thought_chunk":
          appendText(conv, "thought", undefined, contentText(u.content));
          return;
        case "tool_call": {
          conv.blocks.push({
            id: conv.nextId++,
            kind: "tool",
            text: toolTitle(u),
            toolCallId: str(u.toolCallId),
            toolKind: str(u.kind) || undefined,
            status: str(u.status, "pending"),
            parts: toolParts(u.content),
            locations: toolLocations(u.locations),
          });
          return;
        }
        case "tool_call_update": {
          const id = str(u.toolCallId);
          // De trás para frente: o mesmo id pode reaparecer numa conversa longa.
          for (let i = conv.blocks.length - 1; i >= 0; i--) {
            const b = conv.blocks[i];
            if (b.kind !== "tool" || b.toolCallId !== id) continue;
            if (typeof u.status === "string") b.status = u.status;
            if (typeof u.title === "string" && u.title) b.text = u.title;
            // Conteúdo só substitui quando vem algo: um update de status puro
            // não pode apagar o diff que já estava no cartão.
            const parts = toolParts(u.content);
            if (parts.length > 0) b.parts = parts;
            const locs = toolLocations(u.locations);
            if (locs && locs.length > 0) b.locations = locs;
            return;
          }
          return;
        }
        case "available_commands_update":
          conv.commands = list(u.availableCommands).map((cmd) => ({
            name: str(cmd.name),
            description: str(cmd.description),
            input: (cmd.input ?? null) as AcpCommand["input"],
          }));
          return;
        case "current_mode_update":
          conv.modes.current = str(u.currentModeId, conv.modes.current);
          return;
        case "config_option_update": {
          // O agente confirma a troca por aqui; espelha para o seletor não
          // ficar mostrando o valor antigo.
          for (const opt of list(u.configOptions)) {
            const valor = str(opt.currentValue);
            if (!valor) continue;
            if (opt.id === "mode") conv.modes.current = valor;
            if (opt.id === "model") conv.models.current = valor;
          }
          return;
        }
        case "usage_update":
          conv.usage = {
            used: typeof u.used === "number" ? u.used : 0,
            size: typeof u.size === "number" ? u.size : 0,
          };
          return;
        case "plan":
          conv.plan = list(u.entries).map((entry) => ({
            text: str(entry.content),
            status: str(entry.status, "pending"),
          }));
          return;
        default:
          return; // variante que ainda não desenhamos
      }
    }

    case "permission":
      conv.permission = {
        requestId: event.requestId,
        title: toolTitle(event.toolCall ?? {}),
        options: event.options ?? [],
      };
      return;

    case "turnEnded":
      conv.busy = false;
      conv.permission = null;
      // "end_turn" é o caminho feliz e não merece ruído na tela.
      if (event.stopReason && !/endturn/i.test(event.stopReason)) {
        notice(conv, "info", event.stopReason);
      }
      return;

    case "failed":
      conv.busy = false;
      conv.permission = null;
      notice(conv, "error", event.message);
      return;
  }
}

class AcpStore {
  conversations = $state<Record<string, AcpConversation>>({});

  get(paneId: string): AcpConversation {
    return this.conversations[paneId] ?? emptyConversation();
  }

  /** Zera a conversa. Chamado antes de atachar — o daemon reenvia tudo. */
  reset(paneId: string): void {
    this.conversations[paneId] = emptyConversation();
  }

  apply(paneId: string, event: AcpEvent): void {
    const conv = this.conversations[paneId];
    if (!conv) return; // evento de um pane que esta janela não está mostrando
    applyAcpEvent(conv, event);
  }

  /** Eco local do que o usuário mandou (o agente não devolve o próprio prompt). */
  pushUserPrompt(paneId: string, text: string, images: AcpImage[] = []): void {
    const conv = this.conversations[paneId];
    if (!conv) return;
    // As imagens entram como markdown para o mesmo renderizador desenhar; é o
    // eco do que foi enviado, então tem que aparecer igual ao que o agente viu.
    const anexos = images
      .map((img) => `\n\n![](data:${img.mimeType};base64,${img.dataB64})`)
      .join("");
    conv.blocks.push({
      id: conv.nextId++,
      kind: "message",
      role: "user",
      text: text + anexos,
    });
    conv.busy = true;
    conv.plan = [];
  }

  clearPermission(paneId: string): void {
    const conv = this.conversations[paneId];
    if (conv) conv.permission = null;
  }

  /** Otimista: reflete a escolha antes de o agente confirmar. */
  setMode(paneId: string, modeId: string): void {
    const conv = this.conversations[paneId];
    if (conv) conv.modes.current = modeId;
  }

  setModel(paneId: string, modelId: string): void {
    const conv = this.conversations[paneId];
    if (conv) conv.models.current = modelId;
  }

  forget(paneId: string): void {
    delete this.conversations[paneId];
  }
}

export const acp = new AcpStore();
