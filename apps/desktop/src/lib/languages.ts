// Detecção de linguagem para o editor.
//
// Duas camadas, nessa ordem de preferência:
//  1. Pacotes oficiais `@codemirror/lang-*` — parser Lezer de verdade, com
//     indentação e folding corretos.
//  2. `@codemirror/legacy-modes` — modos do CodeMirror 5 embrulhados em
//     StreamLanguage. Highlight só por regex/estado, mas cobre a cauda longa
//     (toml, shell, dockerfile, lua…) sem custo de bundle relevante.
//
// Por que não tree-sitter (o que o Zed usa): as gramáticas do Zed são compiladas
// nativas, não há .wasm pronto pra webview, e o caminho web (web-tree-sitter +
// tree-sitter-wasms) passa de 50 MB — contra um bundle inteiro de ~1,4 MB e um
// alvo de 150 MB de RAM. As cores dos temas do Zed nós importamos assim mesmo
// (ver theme.ts); o que não dá é carregar as gramáticas dele.

import { StreamLanguage } from "@codemirror/language";
import type { Extension } from "@codemirror/state";

import { javascript } from "@codemirror/lang-javascript";
import { json } from "@codemirror/lang-json";
import { markdown } from "@codemirror/lang-markdown";
import { html } from "@codemirror/lang-html";
import { css } from "@codemirror/lang-css";
import { rust } from "@codemirror/lang-rust";
import { python } from "@codemirror/lang-python";
import { go } from "@codemirror/lang-go";
import { yaml } from "@codemirror/lang-yaml";
import { java } from "@codemirror/lang-java";
import { cpp } from "@codemirror/lang-cpp";
import { php } from "@codemirror/lang-php";
import { sql } from "@codemirror/lang-sql";
import { xml } from "@codemirror/lang-xml";

import { toml } from "@codemirror/legacy-modes/mode/toml";
import { shell } from "@codemirror/legacy-modes/mode/shell";
import { dockerFile } from "@codemirror/legacy-modes/mode/dockerfile";
import { lua } from "@codemirror/legacy-modes/mode/lua";
import { ruby } from "@codemirror/legacy-modes/mode/ruby";
import { swift } from "@codemirror/legacy-modes/mode/swift";
import { csharp, kotlin, scala, dart, objectiveC } from "@codemirror/legacy-modes/mode/clike";
import { perl } from "@codemirror/legacy-modes/mode/perl";
import { r } from "@codemirror/legacy-modes/mode/r";
import { haskell } from "@codemirror/legacy-modes/mode/haskell";
import { clojure } from "@codemirror/legacy-modes/mode/clojure";
import { erlang } from "@codemirror/legacy-modes/mode/erlang";
import { elm } from "@codemirror/legacy-modes/mode/elm";
import { nginx } from "@codemirror/legacy-modes/mode/nginx";
import { properties } from "@codemirror/legacy-modes/mode/properties";
import { diff } from "@codemirror/legacy-modes/mode/diff";
import { stex } from "@codemirror/legacy-modes/mode/stex";
import { vb } from "@codemirror/legacy-modes/mode/vb";
import { powerShell } from "@codemirror/legacy-modes/mode/powershell";

const legacy = (mode: Parameters<typeof StreamLanguage.define>[0]): Extension =>
  StreamLanguage.define(mode);

/** extensão de arquivo (sem ponto, minúscula) → extensão do CodeMirror. */
const BY_EXTENSION: Record<string, () => Extension> = {
  // JavaScript e parentes
  js: () => javascript(),
  jsx: () => javascript({ jsx: true }),
  mjs: () => javascript(),
  cjs: () => javascript(),
  ts: () => javascript({ typescript: true }),
  tsx: () => javascript({ typescript: true, jsx: true }),
  mts: () => javascript({ typescript: true }),
  cts: () => javascript({ typescript: true }),

  // Dados e config
  json: () => json(),
  jsonc: () => json(),
  yaml: () => yaml(),
  yml: () => yaml(),
  toml: () => legacy(toml),
  ini: () => legacy(properties),
  cfg: () => legacy(properties),
  conf: () => legacy(properties),
  properties: () => legacy(properties),
  env: () => legacy(properties),
  xml: () => xml(),
  svg: () => xml(),
  plist: () => xml(),
  sql: () => sql(),

  // Web
  html: () => html(),
  htm: () => html(),
  // Svelte e Vue são HTML + blocos de script/estilo; o parser de HTML já lida
  // com <script>/<style>, então o resultado é bem melhor que texto puro.
  svelte: () => html(),
  vue: () => html(),
  css: () => css(),
  scss: () => css(),
  sass: () => css(),
  less: () => css(),

  // Compiladas
  rs: () => rust(),
  go: () => go(),
  java: () => java(),
  kt: () => legacy(kotlin),
  kts: () => legacy(kotlin),
  scala: () => legacy(scala),
  c: () => cpp(),
  h: () => cpp(),
  cc: () => cpp(),
  cpp: () => cpp(),
  cxx: () => cpp(),
  hpp: () => cpp(),
  hh: () => cpp(),
  m: () => legacy(objectiveC),
  mm: () => legacy(objectiveC),
  cs: () => legacy(csharp),
  swift: () => legacy(swift),
  dart: () => legacy(dart),
  hs: () => legacy(haskell),
  erl: () => legacy(erlang),
  hrl: () => legacy(erlang),
  // Elixir fica de fora de propósito: legacy-modes não tem modo pra ela e o de
  // Erlang colore errado (sintaxes diferentes). Texto puro é melhor que mentira.
  elm: () => legacy(elm),
  clj: () => legacy(clojure),
  cljs: () => legacy(clojure),
  edn: () => legacy(clojure),

  // Scripting
  py: () => python(),
  pyi: () => python(),
  rb: () => legacy(ruby),
  php: () => php(),
  pl: () => legacy(perl),
  pm: () => legacy(perl),
  r: () => legacy(r),
  lua: () => legacy(lua),
  vb: () => legacy(vb),
  sh: () => legacy(shell),
  bash: () => legacy(shell),
  zsh: () => legacy(shell),
  fish: () => legacy(shell),
  ps1: () => legacy(powerShell),
  psm1: () => legacy(powerShell),

  // Texto
  md: () => markdown(),
  markdown: () => markdown(),
  mdx: () => markdown(),
  tex: () => legacy(stex),
  diff: () => legacy(diff),
  patch: () => legacy(diff),
};

/** Arquivos que se identificam pelo nome, não pela extensão. */
const BY_FILENAME: Record<string, () => Extension> = {
  dockerfile: () => legacy(dockerFile),
  containerfile: () => legacy(dockerFile),
  makefile: () => legacy(properties),
  gnumakefile: () => legacy(properties),
  "nginx.conf": () => legacy(nginx),
  ".env": () => legacy(properties),
  ".gitignore": () => legacy(properties),
  ".dockerignore": () => legacy(properties),
  ".npmrc": () => legacy(properties),
  ".editorconfig": () => legacy(properties),
  "cargo.lock": () => legacy(toml),
  gemfile: () => legacy(ruby),
  rakefile: () => legacy(ruby),
};

/**
 * Extensões de linguagem para um arquivo. Vazio = sem highlight (texto puro),
 * que continua sendo o fallback correto para binário/desconhecido.
 */
export function langFor(filename: string): Extension[] {
  const base = filename.split(/[/\\]/).pop()?.toLowerCase() ?? "";

  const byName = BY_FILENAME[base];
  if (byName) return [byName()];

  // `Dockerfile.dev`, `Makefile.local` etc. também casam pelo prefixo.
  const prefix = base.split(".")[0];
  const byPrefix = BY_FILENAME[prefix];
  if (byPrefix && base.startsWith(prefix + ".")) return [byPrefix()];

  const ext = base.includes(".") ? base.split(".").pop()! : "";
  const byExt = BY_EXTENSION[ext];
  return byExt ? [byExt()] : [];
}

/** Só para os testes/depuração: quantas extensões o editor reconhece. */
export const SUPPORTED_EXTENSIONS = Object.keys(BY_EXTENSION);
