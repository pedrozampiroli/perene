// O `name` que o agente manda é genérico ("Sonnet") e não diz a versão; ela vem
// no começo da descrição. Sem isto o usuário escolhe modelo às cegas.

import { describe, expect, it } from "vitest";
import { modelLabel } from "./acp.svelte";

describe("modelLabel", () => {
  it("mostra a versão que está na descrição, não o nome genérico", () => {
    expect(
      modelLabel({ id: "sonnet", name: "Sonnet", description: "Sonnet 4.6 · Best for everyday tasks" }),
    ).toBe("Sonnet 4.6");
    expect(
      modelLabel({
        id: "default",
        name: "Default (recommended)",
        description: "Opus 4.6 with 1M context · Most capable for complex work",
      }),
    ).toBe("Opus 4.6 with 1M context");
  });

  it("sem descrição, cai no nome", () => {
    expect(modelLabel({ id: "fable", name: "fable", description: null })).toBe("fable");
  });

  it("sem nome nem descrição, mostra o id em vez de vazio", () => {
    expect(modelLabel({ id: "x", name: "", description: null })).toBe("x");
  });
});
