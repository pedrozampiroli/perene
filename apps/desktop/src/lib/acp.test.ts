// Escolher modelo no chat ACP.
//
// A lista que o adapter anuncia é curta E desatualizada: ele embute um SDK
// anterior ao Opus 5, então omite famílias (opus, fable) e informa versão errada
// para as que conhece — chama o alias `sonnet` de "Sonnet 4.6". Estes testes
// fixam as duas decisões que saem daí.

import { describe, expect, it } from "vitest";
import { applyAcpEvent, emptyConversation, modelLabel } from "./acp.svelte";

const ready = (models: unknown) => {
  const conv = emptyConversation();
  applyAcpEvent(conv, {
    kind: "ready",
    sessionId: "s1",
    modes: null,
    models: models as never,
  });
  return conv.models;
};

/** O que o adapter realmente devolve hoje (capturado do adapter real). */
const DO_ADAPTER = {
  currentModelId: "default",
  availableModels: [
    {
      modelId: "default",
      name: "Default (recommended)",
      description: "Opus 4.6 with 1M context · Most capable for complex work",
    },
    { modelId: "sonnet", name: "Sonnet", description: "Sonnet 4.6 · Best for everyday tasks" },
    { modelId: "haiku", name: "Haiku", description: "Haiku 4.5 · Fastest for quick answers" },
  ],
};

describe("lista de modelos", () => {
  it("acrescenta as famílias que o adapter não anuncia", () => {
    // Sem isto não dá para escolher Opus nem Fable pela interface, embora
    // `session/set_model` aceite os dois (testado contra o adapter real).
    const ids = ready(DO_ADAPTER).available.map((m) => m.id);
    expect(ids).toContain("opus");
    expect(ids).toContain("fable");
  });

  it("não duplica alias que o adapter já anunciou", () => {
    expect(ready(DO_ADAPTER).available.filter((m) => m.id === "sonnet")).toHaveLength(1);

    const soOpus = ready({
      currentModelId: "opus",
      availableModels: [{ modelId: "opus", name: "Opus", description: null }],
    });
    expect(soOpus.available.filter((m) => m.id === "opus")).toHaveLength(1);
  });

  it("preserva o que o adapter mandou", () => {
    const ids = ready(DO_ADAPTER).available.map((m) => m.id);
    expect(ids).toContain("default");
    expect(ids).toContain("haiku");
  });
});

describe("modelLabel", () => {
  it("alias não mostra versão — a que o adapter informa está errada", () => {
    // O alias resolve para o mais recente da família; dizer "4.6" seria mentira.
    expect(
      modelLabel({ id: "sonnet", name: "Sonnet", description: "Sonnet 4.6 · Best for everyday" }),
    ).toBe("Sonnet");
    expect(modelLabel({ id: "opus", name: "opus", description: null })).toBe("Opus");
    expect(modelLabel({ id: "fable", name: "fable", description: null })).toBe("Fable");
  });

  it("o que NÃO é alias mantém a descrição — ali ela é informativa", () => {
    expect(
      modelLabel({
        id: "default",
        name: "Default (recommended)",
        description: "Opus 4.6 with 1M context · Most capable",
      }),
    ).toBe("Opus 4.6 with 1M context");
  });

  it("sem nome nem descrição, mostra o id em vez de vazio", () => {
    expect(modelLabel({ id: "claude-opus-5", name: "", description: null })).toBe("claude-opus-5");
  });
});
