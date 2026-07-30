// Um pane de terminal: xterm.js ligado ao PTY do daemon via comandos Tauri.
//
// Critérios de aceite cobertos (M0) + paste de imagem (M3):
//  - dead keys/IME: xterm usa <textarea> oculto com composition events; NÃO
//    tratamos Option como Meta (macOptionIsMeta:false).
//  - shift+enter: envia LF (\n) — o Claude Code trata como quebra de linha.
//  - copiar: Cmd+C (mac) / Ctrl+Shift+C (win/linux) via plugin de clipboard.
//  - colar texto: evento nativo `paste` do webview → xterm cola (bracketed).
//  - colar imagem: intercepta o `paste`, salva PNG em ~/.perene2/paste/ e escreve
//    o caminho no terminal.
//
// dispose() NÃO mata o PTY (só o xterm local): reorganizar splits/trocar de aba
// preserva a sessão no daemon. Fechar o pane de fato chama terminal_kill à parte.

import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebglAddon } from "@xterm/addon-webgl";
import { Unicode11Addon } from "@xterm/addon-unicode11";
import { ClipboardAddon } from "@xterm/addon-clipboard";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { api } from "./api";
import { t } from "./i18n.svelte";
import { PTY_OUTPUT, PTY_EXIT } from "./events";

const isMac = navigator.userAgent.toLowerCase().includes("mac");

const DARK_PLUS = {
  background: "#1e1e1e",
  foreground: "#d4d4d4",
  cursor: "#d4d4d4",
  cursorAccent: "#1e1e1e",
  selectionBackground: "#264f78",
  black: "#000000",
  red: "#cd3131",
  green: "#0dbc79",
  yellow: "#e5e510",
  blue: "#2472c8",
  magenta: "#bc3fbc",
  cyan: "#11a8cd",
  white: "#e5e5e5",
  brightBlack: "#666666",
  brightRed: "#f14c4c",
  brightGreen: "#23d18b",
  brightYellow: "#f5f543",
  brightBlue: "#3b8eea",
  brightMagenta: "#d670d6",
  brightCyan: "#29b8db",
  brightWhite: "#ffffff",
};

function b64ToBytes(b64: string): Uint8Array {
  const bin = atob(b64);
  const bytes = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
  return bytes;
}

function bytesToB64(bytes: Uint8Array): string {
  let bin = "";
  const chunk = 0x8000;
  for (let i = 0; i < bytes.length; i += chunk) {
    bin += String.fromCharCode(...bytes.subarray(i, i + chunk));
  }
  return btoa(bin);
}

/** Garante PNG: usa os bytes direto se já for PNG, senão converte via canvas. */
async function ensurePng(blob: Blob): Promise<Uint8Array> {
  if (blob.type === "image/png") {
    return new Uint8Array(await blob.arrayBuffer());
  }
  const bmp = await createImageBitmap(blob);
  const canvas = document.createElement("canvas");
  canvas.width = bmp.width;
  canvas.height = bmp.height;
  canvas.getContext("2d")!.drawImage(bmp, 0, 0);
  const png: Blob = await new Promise((res) => canvas.toBlob((b) => res(b!), "image/png"));
  return new Uint8Array(await png.arrayBuffer());
}

export interface PaneOptions {
  cwd?: string | null;
  command?: string | null;
  fontSize?: number;
  webgl?: boolean;
  shell?: string | null;
}

export class PerenePane {
  readonly paneId: string;
  readonly term: Terminal;
  private fit = new FitAddon();
  private unlisteners: UnlistenFn[] = [];
  private resizeObserver?: ResizeObserver;
  private container?: HTMLElement;
  private disposed = false;
  /** Já reportamos falha neste pane? Evita repetir o aviso a cada tecla. */
  private broken = false;

  constructor(paneId: string, fontSize = 13) {
    this.paneId = paneId;
    this.term = new Terminal({
      fontFamily: "Menlo, Monaco, 'DejaVu Sans Mono', 'Courier New', monospace",
      fontSize,
      lineHeight: 1.0,
      cursorBlink: true,
      scrollback: 10_000,
      allowProposedApi: true,
      macOptionIsMeta: false,
      theme: DARK_PLUS,
    });
  }

  async open(container: HTMLElement, opts: PaneOptions = {}): Promise<void> {
    this.container = container;
    this.term.loadAddon(this.fit);
    this.term.loadAddon(new Unicode11Addon());
    this.term.unicode.activeVersion = "11";
    this.term.loadAddon(new ClipboardAddon());

    this.term.open(container);
    // WebGL só quando o usuário liga nas configurações: com 5 terminais o
    // WebContent passava de 270 MB (estoura o alvo de RAM). Padrão = renderer
    // leve do xterm.
    if (opts.webgl) this.tryEnableWebgl();

    this.term.onResize(({ cols, rows }) => {
      void invoke("terminal_resize", { paneId: this.paneId, cols, rows });
    });
    this.fit.fit();

    this.installKeyHandler();
    container.addEventListener("paste", this.onPaste, true);

    this.term.onData((data) => this.send(data));

    this.unlisteners.push(
      await listen<{ paneId: string; dataB64: string }>(PTY_OUTPUT, (e) => {
        if (e.payload.paneId !== this.paneId) return;
        this.term.write(b64ToBytes(e.payload.dataB64));
      }),
    );
    this.unlisteners.push(
      await listen<{ paneId: string; code: number | null }>(PTY_EXIT, (e) => {
        if (e.payload.paneId !== this.paneId) return;
        this.term.writeln(`\r\n\x1b[90m${t("terminal.processEnded")}\x1b[0m`);
      }),
    );

    try {
      await invoke("terminal_spawn", {
        req: {
          paneId: this.paneId,
          cols: this.term.cols,
          rows: this.term.rows,
          cwd: opts.cwd ?? null,
          command: opts.command ?? null,
          shell: opts.shell ?? null,
        },
      });
    } catch (err) {
      // Sem isto o pane fica um retângulo preto mudo: nada aparece e digitar não
      // faz nada. O erro do daemon TEM que chegar ao usuário.
      this.reportBroken(t("terminal.spawnFailed"), err);
    }

    this.resizeObserver = new ResizeObserver(() => this.safeFit());
    this.resizeObserver.observe(container);
  }

  focus(): void {
    this.term.focus();
  }

  refit(): void {
    this.safeFit();
  }

  private tryEnableWebgl(): void {
    try {
      const webgl = new WebglAddon();
      webgl.onContextLoss(() => webgl.dispose());
      this.term.loadAddon(webgl);
    } catch {
      // renderer DOM padrão.
    }
  }

  private installKeyHandler(): void {
    this.term.attachCustomKeyEventHandler((e) => {
      if (e.type !== "keydown") return true;

      const copyCombo = isMac
        ? e.metaKey && !e.shiftKey && e.code === "KeyC"
        : e.ctrlKey && e.shiftKey && e.code === "KeyC";
      if (copyCombo && this.term.hasSelection()) {
        void writeText(this.term.getSelection());
        return false;
      }
      // Shift+Enter → nova linha. O Claude Code (e apps CSI-u) esperam a
      // sequência CSI u `ESC [ 13 ; 2 u`, NÃO um simples \n (o terminal clássico
      // não distingue Enter de Shift+Enter). Fallback universal: Ctrl+J.
      if (e.key === "Enter" && e.shiftKey) {
        e.preventDefault();
        this.send("\x1b[13;2u");
        return false;
      }
      // Colar (texto) fica com o handler nativo de `paste` do webview.
      return true;
    });
  }

  private onPaste = async (e: ClipboardEvent): Promise<void> => {
    const items = e.clipboardData?.items;
    if (!items) return;
    let image: DataTransferItem | undefined;
    for (const it of items) {
      if (it.type.startsWith("image/")) {
        image = it;
        break;
      }
    }
    if (!image) return; // texto → deixa o xterm colar normalmente
    e.preventDefault();
    e.stopPropagation();
    const blob = image.getAsFile();
    if (!blob) return;
    try {
      const png = await ensurePng(blob);
      const path = await api.savePasteImage(bytesToB64(png));
      this.send(path + " ");
    } catch {
      // silencioso: se falhar, nada é colado.
    }
  };

  private send(data: string): void {
    void invoke("terminal_write", { paneId: this.paneId, data }).catch((err) =>
      this.reportBroken(t("terminal.writeFailed"), err),
    );
  }

  /** Escreve o erro no próprio terminal (uma vez só) em vez de sumir com ele. */
  private reportBroken(message: string, err: unknown): void {
    if (this.broken || this.disposed) return;
    this.broken = true;
    const detail = err instanceof Error ? err.message : String(err);
    this.term.write(`\r\n\x1b[31m${message}\x1b[0m\r\n\x1b[90m${detail}\x1b[0m\r\n`);
  }

  private safeFit(): void {
    if (this.disposed) return;
    try {
      this.fit.fit();
    } catch {
      // container oculto/0x0 momentaneamente.
    }
  }

  /** Encerra o xterm local. NÃO mata o PTY (isso é feito no fechamento do pane). */
  dispose(): void {
    this.disposed = true;
    this.resizeObserver?.disconnect();
    this.container?.removeEventListener("paste", this.onPaste, true);
    for (const un of this.unlisteners) un();
    this.unlisteners = [];
    this.term.dispose();
  }
}
