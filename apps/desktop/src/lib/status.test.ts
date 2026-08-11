// Agregação dos indicadores por aba, pasta e workspace.
//
// A ordem é por URGÊNCIA, não por gravidade: `waiting` (esperando aprovação)
// vem antes de `running`, porque é o único estado que precisa de você. É isso
// que faz o indicador do workspace valer a pena — o workspace que não está na
// tela é justamente o que você esqueceu esperando.

import { describe, expect, it } from "vitest";
import type { PaneState } from "./types";
import { worstState as worstOf } from "./status";

describe("prioridade dos estados", () => {
  it("esperando aprovação ganha de rodando", () => {
    expect(worstOf(["running", "waiting"])).toBe("waiting");
  });

  it("erro ganha de tudo", () => {
    expect(worstOf(["done", "running", "waiting", "error"])).toBe("error");
  });

  it("pronto só aparece quando nada mais está acontecendo", () => {
    expect(worstOf(["done"])).toBe("done");
    expect(worstOf(["done", "running"])).toBe("running");
  });

  it("sem estado, não mostra nada", () => {
    expect(worstOf([])).toBeNull();
    expect(worstOf([undefined, undefined])).toBeNull();
  });

  it("um pane esperando pinta o workspace inteiro", () => {
    // Muitas abas ociosas + uma pedindo aprovação: o workspace tem que avisar.
    const estados = [...Array(20).fill(undefined), "waiting" as PaneState];
    expect(worstOf(estados)).toBe("waiting");
  });
});
