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

import type { AcpEvent, AcpPermissionOption } from "./types";

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
  /** Só em `notice`. */
  level?: "error" | "info";
}

export interface AcpPending {
  requestId: number;
  title: string;
  options: AcpPermissionOption[];
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
  nextId: number;
}

export function emptyConversation(): AcpConversation {
  return { ready: false, busy: false, blocks: [], plan: [], permission: null, nextId: 1 };
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

/** Acrescenta texto ao último bloco do mesmo tipo, ou abre um novo. */
function appendText(conv: AcpConversation, kind: AcpBlock["kind"], role: AcpBlock["role"], text: string) {
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

/** Aplica um evento do daemon à conversa. Muta (o `$state` é reativo em profundidade). */
export function applyAcpEvent(conv: AcpConversation, event: AcpEvent): void {
  switch (event.kind) {
    case "ready":
      conv.ready = true;
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
          const id = str(u.toolCallId);
          conv.blocks.push({
            id: conv.nextId++,
            kind: "tool",
            text: toolTitle(u),
            toolCallId: id,
            toolKind: str(u.kind) || undefined,
            status: str(u.status, "pending"),
          });
          return;
        }
        case "tool_call_update": {
          const id = str(u.toolCallId);
          // De trás para frente: o mesmo id pode reaparecer numa conversa longa.
          for (let i = conv.blocks.length - 1; i >= 0; i--) {
            const b = conv.blocks[i];
            if (b.kind === "tool" && b.toolCallId === id) {
              if (typeof u.status === "string") b.status = u.status;
              return;
            }
          }
          return;
        }
        case "plan":
          conv.plan = (Array.isArray(u.entries) ? u.entries : []).map((e) => {
            const entry = (e ?? {}) as Record<string, unknown>;
            return { text: str(entry.content), status: str(entry.status, "pending") };
          });
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
  pushUserPrompt(paneId: string, text: string): void {
    const conv = this.conversations[paneId];
    if (!conv) return;
    conv.blocks.push({ id: conv.nextId++, kind: "message", role: "user", text });
    conv.busy = true;
    conv.plan = [];
  }

  clearPermission(paneId: string): void {
    const conv = this.conversations[paneId];
    if (conv) conv.permission = null;
  }

  forget(paneId: string): void {
    delete this.conversations[paneId];
  }
}

export const acp = new AcpStore();
