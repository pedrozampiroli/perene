// Prioridade dos indicadores de sessão.
//
// Módulo próprio (e não um método do store) para a regra ser testável sem
// montar o app: o store importa APIs do Tauri, que não existem no vitest.

import type { PaneState } from "./types";

/**
 * Ordem por **urgência**, não por gravidade.
 *
 * `waiting` (esperando aprovação) vem antes de `running` porque é o único
 * estado que precisa de você — o resto é informativo. É isso que dá valor ao
 * indicador no workspace: o que não está na tela é justamente aquele cuja
 * sessão você esqueceu esperando.
 */
const ORDEM: PaneState[] = ["error", "waiting", "running", "done"];

/** O estado mais urgente do conjunto, ou `null` se não há nada a mostrar. */
export function worstState(estados: (PaneState | undefined)[]): PaneState | null {
  for (const st of ORDEM) {
    if (estados.includes(st)) return st;
  }
  return null;
}
