// Editor CodeMirror 6: numeração de linhas, busca/substituição (⌘F/⌘⇧F via
// basicSetup), ⌘S salva, syntax highlight tema dark (one-dark ≈ Dark+).

import { EditorView, keymap } from "@codemirror/view";
import { EditorState, type Extension } from "@codemirror/state";
import { MergeView } from "@codemirror/merge";
import { basicSetup } from "codemirror";
import { indentWithTab } from "@codemirror/commands";
import { oneDark } from "@codemirror/theme-one-dark";
import { javascript } from "@codemirror/lang-javascript";
import { json } from "@codemirror/lang-json";
import { markdown } from "@codemirror/lang-markdown";
import { html } from "@codemirror/lang-html";
import { css } from "@codemirror/lang-css";
import { rust } from "@codemirror/lang-rust";
import { python } from "@codemirror/lang-python";

function langFor(filename: string): Extension[] {
  const ext = filename.split(".").pop()?.toLowerCase();
  switch (ext) {
    case "js":
    case "jsx":
    case "mjs":
    case "cjs":
      return [javascript()];
    case "ts":
    case "tsx":
      return [javascript({ typescript: true })];
    case "json":
      return [json()];
    case "md":
    case "markdown":
      return [markdown()];
    case "html":
    case "htm":
    case "svelte":
    case "vue":
      return [html()];
    case "css":
    case "scss":
    case "less":
      return [css()];
    case "rs":
      return [rust()];
    case "py":
      return [python()];
    default:
      return [];
  }
}

/** Estado de edição de um arquivo (para o editor multi-abas trocar via setState,
 *  preservando undo/cursor por arquivo). onDirty marca a aba; ⌘S salva. */
export function createFileState(
  content: string,
  filename: string,
  onDirty: () => void,
  onSave: (content: string) => void,
): EditorState {
  return EditorState.create({
    doc: content,
    extensions: [
      basicSetup,
      oneDark,
      keymap.of([indentWithTab]),
      keymap.of([
        {
          key: "Mod-s",
          preventDefault: true,
          run: (view) => {
            onSave(view.state.doc.toString());
            return true;
          },
        },
      ]),
      EditorView.updateListener.of((u) => {
        if (u.docChanged) onDirty();
      }),
      ...langFor(filename),
      EditorView.theme({ "&": { height: "100%" }, ".cm-scroller": { overflow: "auto" } }),
    ],
  });
}

/** Diff lado a lado (split) read-only: `old` (HEAD) à esquerda, `new` à direita. */
export function createMergeView(
  parent: HTMLElement,
  oldDoc: string,
  newDoc: string,
  filename: string,
): MergeView {
  const common: Extension[] = [
    basicSetup,
    oneDark,
    ...langFor(filename),
    EditorState.readOnly.of(true),
    EditorView.editable.of(false),
    EditorView.theme({ "&": { height: "100%" }, ".cm-scroller": { overflow: "auto" } }),
  ];
  return new MergeView({
    a: { doc: oldDoc, extensions: common },
    b: { doc: newDoc, extensions: common },
    parent,
    gutter: true,
    highlightChanges: true,
    collapseUnchanged: { margin: 3, minSize: 4 },
  });
}

export function createEditor(
  parent: HTMLElement,
  doc: string,
  filename: string,
  onSave: (content: string) => void,
): EditorView {
  const save = keymap.of([
    {
      key: "Mod-s",
      preventDefault: true,
      run: (view) => {
        onSave(view.state.doc.toString());
        return true;
      },
    },
  ]);
  const state = EditorState.create({
    doc,
    extensions: [
      basicSetup,
      oneDark,
      keymap.of([indentWithTab]),
      save,
      ...langFor(filename),
      EditorView.theme({ "&": { height: "100%" }, ".cm-scroller": { overflow: "auto" } }),
    ],
  });
  return new EditorView({ state, parent });
}
