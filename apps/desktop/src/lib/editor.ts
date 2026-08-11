// Editor CodeMirror 6: numeração de linhas, busca/substituição (⌘F/⌘⇧F via
// basicSetup), ⌘S salva.
//
// Cores e linguagens saem daqui de propósito: o tema vem de `theme.svelte.ts`
// (o mesmo que pinta a UI e o terminal) e o mapa de linguagens de
// `languages.ts`. Antes eram `oneDark` fixo em três lugares e um switch com 8
// extensões.

import { EditorView, keymap } from "@codemirror/view";
import { Compartment, EditorState, type Extension } from "@codemirror/state";
import { MergeView } from "@codemirror/merge";
import { basicSetup } from "codemirror";
import { indentWithTab } from "@codemirror/commands";

import { langFor } from "./languages";
import { theme, themeExtensions } from "./theme.svelte";

/** O tema fica num compartment pra poder ser trocado num editor JÁ ABERTO —
 *  sem isso, mudar de tema só valeria para arquivos abertos depois. */
const themeCompartment = new Compartment();

/** Views vivas, para o `repaintAll` alcançar todas. */
const liveViews = new Set<EditorView>();

function themeExtension(): Extension {
  return themeCompartment.of(themeExtensions(theme.current));
}

/** Reaplica o tema atual em todos os editores abertos. */
export function repaintEditors(): void {
  const next = themeExtensions(theme.current);
  for (const view of liveViews) {
    view.dispatch({ effects: themeCompartment.reconfigure(next) });
  }
}

/** Registra a view e garante a baixa quando ela morre. */
function track(view: EditorView): EditorView {
  liveViews.add(view);
  const originalDestroy = view.destroy.bind(view);
  view.destroy = () => {
    liveViews.delete(view);
    originalDestroy();
  };
  return view;
}

function saveKeymap(onSave: (content: string) => void): Extension {
  return keymap.of([
    {
      key: "Mod-s",
      preventDefault: true,
      run: (view) => {
        onSave(view.state.doc.toString());
        return true;
      },
    },
  ]);
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
      themeExtension(),
      keymap.of([indentWithTab]),
      saveKeymap(onSave),
      EditorView.updateListener.of((u) => {
        if (u.docChanged) onDirty();
      }),
      ...langFor(filename),
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
    themeExtension(),
    ...langFor(filename),
    EditorState.readOnly.of(true),
    EditorView.editable.of(false),
  ];
  const merge = new MergeView({
    a: { doc: oldDoc, extensions: common },
    b: { doc: newDoc, extensions: common },
    parent,
    gutter: true,
    highlightChanges: true,
    collapseUnchanged: { margin: 3, minSize: 4 },
  });
  track(merge.a);
  track(merge.b);
  return merge;
}

export function createEditor(
  parent: HTMLElement,
  doc: string,
  filename: string,
  onSave: (content: string) => void,
): EditorView {
  const state = EditorState.create({
    doc,
    extensions: [
      basicSetup,
      themeExtension(),
      keymap.of([indentWithTab]),
      saveKeymap(onSave),
      ...langFor(filename),
    ],
  });
  return track(new EditorView({ state, parent }));
}
