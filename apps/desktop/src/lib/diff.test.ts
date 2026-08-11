// O diff é o único lugar do chat com algoritmo de verdade — e é o que o usuário
// olha para decidir se aceita uma edição. Um diff errado aqui é pior que nenhum.

import { describe, expect, it } from "vitest";
import { buildDiff, diffLines, diffStats, withElision } from "./diff";

describe("diffLines", () => {
  it("marca só o que mudou, preservando o contexto", () => {
    const linhas = diffLines(["a", "b", "c"], ["a", "B", "c"]);
    expect(linhas.map((l) => `${l.kind}:${l.text}`)).toEqual([
      "keep:a",
      "del:b",
      "add:B",
      "keep:c",
    ]);
  });

  it("entende inserção no meio sem repintar o resto", () => {
    const linhas = diffLines(["a", "c"], ["a", "b", "c"]);
    expect(diffStats(linhas)).toEqual({ add: 1, del: 0 });
    expect(linhas.filter((l) => l.kind === "keep")).toHaveLength(2);
  });

  it("arquivos idênticos não geram mudança nenhuma", () => {
    const linhas = diffLines(["x", "y"], ["x", "y"]);
    expect(diffStats(linhas)).toEqual({ add: 0, del: 0 });
  });

  it("acima do teto cai no caminho simples em vez de travar", () => {
    // O LCS é O(n·m): 5000×5000 seria 25M de células.
    const a = Array.from({ length: 5000 }, (_, i) => `linha ${i}`);
    const b = [...a, "nova"];
    const inicio = performance.now();
    const linhas = diffLines(a, b);
    expect(performance.now() - inicio).toBeLessThan(500);
    expect(diffStats(linhas)).toEqual({ add: 5001, del: 5000 });
  });
});

describe("withElision", () => {
  it("esconde o contexto longe da mudança", () => {
    const iguais = Array.from({ length: 30 }, (_, i) => ({
      kind: "keep" as const,
      text: `l${i}`,
    }));
    const linhas = withElision([...iguais, { kind: "add", text: "nova" }]);
    const lacunas = linhas.filter((l) => l.kind === "gap");
    expect(lacunas).toHaveLength(1);
    expect(lacunas[0].text).toContain("27");
    // 3 linhas de contexto antes + a mudança.
    expect(linhas.filter((l) => l.kind === "keep")).toHaveLength(3);
  });

  it("não cria lacuna quando tudo é relevante", () => {
    const linhas = withElision([
      { kind: "keep", text: "a" },
      { kind: "add", text: "b" },
    ]);
    expect(linhas.some((l) => l.kind === "gap")).toBe(false);
  });
});

describe("buildDiff", () => {
  it("arquivo novo é tudo adição, sem lacuna", () => {
    const linhas = buildDiff(null, "um\ndois\ntrês");
    expect(linhas.every((l) => l.kind === "add")).toBe(true);
    expect(diffStats(linhas)).toEqual({ add: 3, del: 0 });
  });

  it("arquivo esvaziado conta as remoções", () => {
    const linhas = buildDiff("um\ndois", "");
    expect(diffStats(linhas).del).toBe(2);
  });
});
