// Um pane de terminal: xterm.js ligado ao PTY do backend Rust via comandos Tauri.
//
// Decisões que dão conta dos critérios de aceite do M0:
//  - dead keys/IME (option+e → é): xterm usa um <textarea> oculto com
//    compositionstart/update/end; NÃO tratamos Option como Meta (macOptionIsMeta:
//    false), então o SO compõe o acento normalmente.
//  - shift+enter no Claude Code: enviamos LF (\n) — é o que o `/terminal-setup`
//    do Claude configura; Enter puro manda CR (\r) e submete.
//  - copiar/colar: Cmd+C/Cmd+V (mac) e Ctrl+Shift+C/V (win/linux) via plugin de
//    clipboard do Tauri (robusto entre plataformas); paste passa pelo bracketed
//    paste do xterm.
//  - scroll/seleção/cores/vim: nativos do xterm.js.

import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebglAddon } from "@xterm/addon-webgl";
import { Unicode11Addon } from "@xterm/addon-unicode11";
import { ClipboardAddon } from "@xterm/addon-clipboard";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  readText,
  writeText,
} from "@tauri-apps/plugin-clipboard-manager";
import { PTY_OUTPUT, PTY_EXIT } from "./events";

const isMac = navigator.userAgent.toLowerCase().includes("mac");

// Paleta Dark+ (VSCode) — mesmo tema-alvo do visualizador de arquivos (M5).
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

export interface PaneOptions {
  cwd?: string | null;
  command?: string | null;
}

export class PerenePane {
  readonly paneId: string;
  readonly term: Terminal;
  private fit = new FitAddon();
  private unlisteners: UnlistenFn[] = [];
  private resizeObserver?: ResizeObserver;
  private disposed = false;

  constructor(paneId: string) {
    this.paneId = paneId;
    this.term = new Terminal({
      fontFamily: "Menlo, Monaco, 'DejaVu Sans Mono', 'Courier New', monospace",
      fontSize: 13,
      lineHeight: 1.0,
      cursorBlink: true,
      scrollback: 10_000,
      allowProposedApi: true, // exigido pelo addon unicode11
      macOptionIsMeta: false, // preserva dead keys no mac
      theme: DARK_PLUS,
    });
  }

  async open(container: HTMLElement, opts: PaneOptions = {}): Promise<void> {
    this.term.loadAddon(this.fit);
    this.term.loadAddon(new Unicode11Addon());
    this.term.unicode.activeVersion = "11";
    this.term.loadAddon(new ClipboardAddon());

    this.term.open(container);
    this.tryEnableWebgl();

    // Sincroniza tamanho do PTY com o do xterm.
    this.term.onResize(({ cols, rows }) => {
      void invoke("terminal_resize", { paneId: this.paneId, cols, rows });
    });
    this.fit.fit();

    this.installKeyHandler();

    // Input do usuário → PTY.
    this.term.onData((data) => this.send(data));

    // Output do PTY → xterm (já coalescido e em base64 pelo Rust).
    this.unlisteners.push(
      await listen<{ paneId: string; dataB64: string }>(PTY_OUTPUT, (e) => {
        if (e.payload.paneId !== this.paneId) return;
        this.term.write(b64ToBytes(e.payload.dataB64));
      }),
    );
    this.unlisteners.push(
      await listen<{ paneId: string; code: number | null }>(PTY_EXIT, (e) => {
        if (e.payload.paneId !== this.paneId) return;
        this.term.writeln("\r\n\x1b[90m[processo encerrado]\x1b[0m");
      }),
    );

    // Cria o PTY do lado Rust com o tamanho atual.
    await invoke("terminal_spawn", {
      req: {
        paneId: this.paneId,
        cols: this.term.cols,
        rows: this.term.rows,
        cwd: opts.cwd ?? null,
        command: opts.command ?? null,
      },
    });

    // Reajusta em qualquer mudança de tamanho do container.
    this.resizeObserver = new ResizeObserver(() => this.safeFit());
    this.resizeObserver.observe(container);

    this.term.focus();
  }

  private tryEnableWebgl(): void {
    // WebGL acelera muito, mas o WebKitGTK (Linux) pode não suportar — cai pro
    // renderer DOM sem quebrar.
    try {
      const webgl = new WebglAddon();
      webgl.onContextLoss(() => webgl.dispose());
      this.term.loadAddon(webgl);
    } catch {
      // Segue com o renderer DOM padrão.
    }
  }

  private installKeyHandler(): void {
    this.term.attachCustomKeyEventHandler((e) => {
      if (e.type !== "keydown") return true;

      const copyCombo = isMac
        ? e.metaKey && !e.shiftKey && e.code === "KeyC"
        : e.ctrlKey && e.shiftKey && e.code === "KeyC";
      const pasteCombo = isMac
        ? e.metaKey && !e.shiftKey && e.code === "KeyV"
        : e.ctrlKey && e.shiftKey && e.code === "KeyV";

      if (copyCombo && this.term.hasSelection()) {
        void writeText(this.term.getSelection());
        return false;
      }
      if (pasteCombo) {
        void readText().then((t) => {
          if (t) this.term.paste(t);
        });
        return false;
      }
      // Shift+Enter → nova linha (Claude Code trata \n como quebra).
      if (e.key === "Enter" && e.shiftKey) {
        this.send("\n");
        return false;
      }
      return true;
    });
  }

  private send(data: string): void {
    void invoke("terminal_write", { paneId: this.paneId, data });
  }

  private safeFit(): void {
    if (this.disposed) return;
    try {
      this.fit.fit();
    } catch {
      // container pode estar oculto/0x0 momentaneamente.
    }
  }

  dispose(): void {
    this.disposed = true;
    this.resizeObserver?.disconnect();
    for (const un of this.unlisteners) un();
    this.unlisteners = [];
    void invoke("terminal_kill", { paneId: this.paneId }).catch(() => {});
    this.term.dispose();
  }
}
