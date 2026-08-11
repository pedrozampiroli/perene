// O comando montado aqui é o que roda no terminal do usuário. Errar significa
// abrir a CLI errada, perder conversa ou cair num "session not found" — foi
// exatamente esse tipo de bug que já custou tempo neste projeto.

import { describe, expect, it } from "vitest";
import { buildCommand, supportsAcp, supportsFork } from "./profiles";
import type { Pane, Settings } from "./types";

const settings = (over: Partial<Settings> = {}): Settings =>
  ({ yolo: false, ...over }) as Settings;

const pane = (over: Partial<Pane> = {}): Pane =>
  ({
    id: "pane_1",
    kind: "terminal",
    toolProfileId: "claude",
    workingDirectory: "/tmp",
    harnessSessionId: "abc-123",
    resumeExisting: false,
    forkFromSessionId: null,
    scrollbackFile: null,
    createdAt: 0,
    updatedAt: 0,
    ...over,
  }) as Pane;

describe("fork", () => {
  it("claude bifurca por --fork-session, a partir da sessão de origem", () => {
    const cmd = buildCommand(pane({ forkFromSessionId: "origem-1" }), settings(), true);
    expect(cmd).toContain("claude --resume origem-1 --fork-session");
  });

  it("codex e opencode caem em 'a mais recente' quando não há id", () => {
    // Ao criar sessão, essas duas não fixam id — só o claude usa --session-id.
    const codex = buildCommand(
      pane({ toolProfileId: "codex", harnessSessionId: null, forkFromSessionId: "" }),
      settings(),
      true,
    );
    expect(codex).toContain("codex fork --last");

    const opencode = buildCommand(
      pane({ toolProfileId: "opencode", harnessSessionId: null, forkFromSessionId: "" }),
      settings(),
      true,
    );
    expect(opencode).toContain("opencode --continue --fork");
  });

  it("com id conhecido, bifurca a sessão exata", () => {
    const codex = buildCommand(
      pane({ toolProfileId: "codex", forkFromSessionId: "sess-9" }),
      settings(),
      true,
    );
    expect(codex).toContain("codex fork sess-9");
  });

  it("todo fork tem fallback: sessão sumida não larga o usuário no vazio", () => {
    for (const tool of ["claude", "codex", "opencode"]) {
      const cmd = buildCommand(
        pane({ toolProfileId: tool, forkFromSessionId: "x" }),
        settings(),
        true,
      );
      expect(cmd, tool).toContain("||");
    }
  });

  it("respeita o modo YOLO", () => {
    const cmd = buildCommand(
      pane({ forkFromSessionId: "o1" }),
      settings({ yolo: true }),
      true,
    );
    expect(cmd).toContain("--dangerously-skip-permissions");
  });

  it("shell não bifurca — não há conversa", () => {
    expect(supportsFork("shell")).toBe(false);
    expect(buildCommand(pane({ toolProfileId: "shell", forkFromSessionId: "x" }), settings(), true))
      .toBeNull();
  });

  it("as três CLIs de IA sabem bifurcar; só o claude tem adapter ACP", () => {
    expect(["claude", "codex", "opencode"].every(supportsFork)).toBe(true);
    expect(supportsAcp("claude")).toBe(true);
    expect(supportsAcp("codex")).toBe(false);
  });
});

describe("os outros modos seguem intactos", () => {
  it("pane novo cria sessão em vez de retomar", () => {
    expect(buildCommand(pane(), settings(), true)).toBe("claude --session-id abc-123");
  });

  it("retomada do histórico usa o id exato", () => {
    const cmd = buildCommand(pane({ resumeExisting: true }), settings(), false);
    expect(cmd).toContain("claude --resume abc-123");
    expect(cmd).not.toContain("--fork-session");
  });

  it("nunca usa `claude --continue` (regra do projeto)", () => {
    const casos = [
      buildCommand(pane(), settings(), true),
      buildCommand(pane(), settings(), false),
      buildCommand(pane({ resumeExisting: true }), settings(), false),
      buildCommand(pane({ forkFromSessionId: "o" }), settings(), true),
    ];
    for (const cmd of casos) expect(cmd ?? "").not.toContain("claude --continue");
  });
});
