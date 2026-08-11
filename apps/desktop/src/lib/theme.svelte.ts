// Tema em runtime: aplica os tokens como CSS vars no :root e deriva as duas
// outras superfícies (terminal e editor) do mesmo objeto.
//
// A UI inteira lê `var(--token)`, então trocar de tema é reescrever ~12 vars —
// não há componente pra re-renderizar. Terminal e CodeMirror não leem CSS var,
// esses recebem o tema por API (ver `applyTerminalTheme` em terminal.ts e
// `themeExtensions` aqui).

import { EditorView } from "@codemirror/view";
import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import { tags } from "@lezer/highlight";
import type { Extension } from "@codemirror/state";

import { api } from "./api";
import type { Theme } from "./types";

/** Tema embutido em TS: o front precisa de cores antes do primeiro await. */
const FALLBACK: Theme = {
  id: "dark-plus",
  name: "Dark+",
  light: false,
  ui: {
    bg: "#1e1e1e",
    fg: "#d4d4d4",
    panel: "#252526",
    elevated: "#2d2d30",
    border: "#3c3c3c",
    muted: "#9aa0a6",
    accent: "#0a84ff",
    accentFg: "#ffffff",
    danger: "#f14c4c",
    warning: "#e5e510",
    success: "#23d18b",
    selection: "#264f78",
  },
  terminal: {
    background: "#1e1e1e",
    foreground: "#d4d4d4",
    cursor: "#d4d4d4",
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
  },
  syntax: {
    comment: "#6a9955",
    keyword: "#569cd6",
    string: "#ce9178",
    number: "#b5cea8",
    function: "#dcdcaa",
    typeName: "#4ec9b0",
    variable: "#9cdcfe",
    constant: "#4fc1ff",
    operator: "#d4d4d4",
    punctuation: "#808080",
    property: "#9cdcfe",
    tag: "#569cd6",
  },
};

class ThemeState {
  current = $state<Theme>(FALLBACK);
  available = $state<Theme[]>([]);

  /** Carrega o tema salvo e o catálogo. Chamado uma vez, no boot. */
  async init(id: string): Promise<void> {
    try {
      const [active, all] = await Promise.all([api.themeActive(id), api.themesList()]);
      this.available = all;
      this.apply(active);
    } catch {
      // Sem backend (ou tema corrompido) o app continua com o fallback: ficar
      // sem cor nenhuma seria pior que ficar com o tema padrão.
      this.apply(FALLBACK);
    }
  }

  async refreshCatalog(): Promise<void> {
    try {
      this.available = await api.themesList();
    } catch {
      /* catálogo é secundário */
    }
  }

  /** Troca o tema ativo e repinta a UI. */
  apply(next: Theme): void {
    this.current = next;
    writeCssVars(next);
    // Terminais e editores já abertos não leem CSS var: precisam do tema na
    // mão. Os imports são tardios porque os dois módulos importam este (ciclo).
    void import("./terminal").then((m) => m.PerenePane.repaintAll());
    void import("./editor").then((m) => m.repaintEditors());
  }

  async select(id: string): Promise<void> {
    this.apply(await api.themeActive(id));
  }
}

export const theme = new ThemeState();

/** Escreve os tokens no :root. Mantido fora da classe pra ser testável e pra
 *  deixar explícito que é o ÚNICO ponto que toca no DOM global. */
function writeCssVars(t: Theme): void {
  const root = document.documentElement;
  const set = (k: string, v: string) => root.style.setProperty(k, v);

  set("--bg", t.ui.bg);
  set("--fg", t.ui.fg);
  set("--panel", t.ui.panel);
  set("--elevated", t.ui.elevated);
  set("--border", t.ui.border);
  set("--muted", t.ui.muted);
  set("--accent", t.ui.accent);
  set("--accent-fg", t.ui.accentFg);
  set("--danger", t.ui.danger);
  set("--warning", t.ui.warning);
  set("--success", t.ui.success);
  set("--selection", t.ui.selection);

  // Derivados de conveniência, pra não espalhar rgba() pelos componentes.
  set("--overlay", t.light ? "rgba(0,0,0,.28)" : "rgba(0,0,0,.55)");
  set("--shadow", t.light ? "rgba(0,0,0,.14)" : "rgba(0,0,0,.45)");
  set("--scrollbar", t.ui.border);
  set("--scrollbar-hover", t.ui.muted);

  // `color-scheme` faz o webview escolher a variante certa dos controles
  // nativos (scrollbar, caret, inputs) — sem isso, tema claro fica com
  // scrollbar escura.
  root.style.colorScheme = t.light ? "light" : "dark";
}

/** Paleta no formato que o xterm.js espera. */
export function xtermTheme(t: Theme) {
  return {
    background: t.terminal.background,
    foreground: t.terminal.foreground,
    cursor: t.terminal.cursor,
    cursorAccent: t.terminal.background,
    selectionBackground: t.terminal.selectionBackground,
    black: t.terminal.black,
    red: t.terminal.red,
    green: t.terminal.green,
    yellow: t.terminal.yellow,
    blue: t.terminal.blue,
    magenta: t.terminal.magenta,
    cyan: t.terminal.cyan,
    white: t.terminal.white,
    brightBlack: t.terminal.brightBlack,
    brightRed: t.terminal.brightRed,
    brightGreen: t.terminal.brightGreen,
    brightYellow: t.terminal.brightYellow,
    brightBlue: t.terminal.brightBlue,
    brightMagenta: t.terminal.brightMagenta,
    brightCyan: t.terminal.brightCyan,
    brightWhite: t.terminal.brightWhite,
  };
}

/**
 * Extensões do CodeMirror para um tema: cores do chrome do editor + o
 * `HighlightStyle` da sintaxe.
 *
 * O mapa tag→cor é o que substitui o `oneDark` fixo. Os nomes de token do
 * `Theme.syntax` seguem os captures do Zed justamente pra um tema importado de
 * lá cair aqui sem tradução extra.
 */
export function themeExtensions(th: Theme): Extension[] {
  const s = th.syntax;

  // Cada token do tema cobre uma família de tags do Lezer. Ser generoso aqui é o
  // que faz linguagem de legacy-mode (Go, shell, toml) sair colorida: elas
  // emitem tags básicas, não as especializadas.
  const highlight = HighlightStyle.define([
    // `meta` entra aqui por causa do shebang (`#!/bin/bash`), que os modos de
    // shell emitem como meta e o VS Code colore como comentário.
    { tag: [tags.comment, tags.lineComment, tags.blockComment, tags.docComment, tags.meta], color: s.comment, fontStyle: "italic" },
    {
      tag: [tags.keyword, tags.modifier, tags.controlKeyword, tags.moduleKeyword, tags.definitionKeyword, tags.operatorKeyword, tags.self, tags.null],
      color: s.keyword,
    },
    { tag: [tags.string, tags.special(tags.string), tags.regexp, tags.character], color: s.string },
    { tag: [tags.number, tags.integer, tags.float, tags.unit], color: s.number },
    { tag: [tags.function(tags.variableName), tags.function(tags.propertyName), tags.macroName, tags.labelName], color: s.function },
    { tag: [tags.typeName, tags.className, tags.namespace, tags.annotation, tags.standard(tags.typeName)], color: s.typeName },
    { tag: [tags.variableName, tags.definition(tags.variableName), tags.local(tags.variableName)], color: s.variable },
    { tag: [tags.bool, tags.constant(tags.variableName), tags.standard(tags.variableName), tags.atom, tags.literal], color: s.constant },
    { tag: [tags.operator, tags.derefOperator, tags.arithmeticOperator, tags.logicOperator, tags.compareOperator, tags.updateOperator, tags.definitionOperator], color: s.operator },
    { tag: [tags.punctuation, tags.separator, tags.bracket, tags.paren, tags.brace, tags.squareBracket, tags.angleBracket], color: s.punctuation },
    { tag: [tags.propertyName, tags.definition(tags.propertyName), tags.attributeName], color: s.property },
    { tag: [tags.tagName, tags.angleBracket, tags.documentMeta, tags.processingInstruction], color: s.tag },
    { tag: [tags.link, tags.url], color: th.ui.accent, textDecoration: "underline" },
    { tag: [tags.heading], color: s.function, fontWeight: "bold" },
    { tag: [tags.emphasis], fontStyle: "italic" },
    { tag: [tags.strong], fontWeight: "bold" },
    { tag: [tags.strikethrough], textDecoration: "line-through" },
    { tag: [tags.invalid], color: th.ui.danger },
    { tag: [tags.inserted], color: th.ui.success },
    { tag: [tags.deleted], color: th.ui.danger },
  ]);

  const chrome = EditorView.theme(
    {
      "&": { height: "100%", color: th.ui.fg, backgroundColor: th.ui.bg },
      ".cm-scroller": { overflow: "auto" },
      ".cm-content": { caretColor: th.ui.fg },
      ".cm-cursor, .cm-dropCursor": { borderLeftColor: th.ui.fg },
      "&.cm-focused .cm-selectionBackground, .cm-selectionBackground, .cm-content ::selection": {
        backgroundColor: th.ui.selection,
      },
      ".cm-gutters": {
        backgroundColor: th.ui.bg,
        color: th.ui.muted,
        border: "none",
        borderRight: `1px solid ${th.ui.border}`,
      },
      ".cm-activeLine": { backgroundColor: th.ui.elevated },
      ".cm-activeLineGutter": { backgroundColor: th.ui.elevated, color: th.ui.fg },
      ".cm-selectionMatch": { backgroundColor: th.ui.selection },
      ".cm-searchMatch": { backgroundColor: th.ui.selection, outline: `1px solid ${th.ui.accent}` },
      ".cm-searchMatch.cm-searchMatch-selected": { backgroundColor: th.ui.accent },
      ".cm-panels": { backgroundColor: th.ui.panel, color: th.ui.fg },
      ".cm-panels input, .cm-panels button": {
        backgroundColor: th.ui.elevated,
        color: th.ui.fg,
        border: `1px solid ${th.ui.border}`,
      },
      ".cm-tooltip": {
        backgroundColor: th.ui.panel,
        color: th.ui.fg,
        border: `1px solid ${th.ui.border}`,
      },
      ".cm-foldPlaceholder": {
        backgroundColor: th.ui.elevated,
        color: th.ui.muted,
        border: "none",
      },
    },
    { dark: !th.light },
  );

  return [chrome, syntaxHighlighting(highlight)];
}
