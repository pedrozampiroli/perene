// Diff por linha para os cartões de ferramenta do chat ACP.
//
// Implementação própria (LCS por linha) em vez do CodeMirror merge do editor:
// aqui são trechos curtos dentro de cartões, muitos por conversa. Montar um
// editor completo para cada um custaria caro em RAM — e o alvo do projeto é
// < 150 MB com 5 sessões.

export type DiffLine =
  | { kind: "keep" | "add" | "del"; text: string }
  /** Bloco de contexto omitido. */
  | { kind: "gap"; text: string };

/** Acima disto o LCS (O(n·m)) sai caro; cai no diff burro, que ainda é legível. */
const MAX_LCS_LINES = 400;
/** Linhas de contexto mantidas de cada lado de uma mudança. */
const CONTEXT = 3;

/** Maior subsequência comum por linha. */
export function diffLines(a: string[], b: string[]): DiffLine[] {
  if (a.length > MAX_LCS_LINES || b.length > MAX_LCS_LINES) {
    return [
      ...a.map((text): DiffLine => ({ kind: "del", text })),
      ...b.map((text): DiffLine => ({ kind: "add", text })),
    ];
  }
  const n = a.length;
  const m = b.length;
  const dp: number[][] = Array.from({ length: n + 1 }, () => new Array(m + 1).fill(0));
  for (let i = n - 1; i >= 0; i--) {
    for (let j = m - 1; j >= 0; j--) {
      dp[i][j] = a[i] === b[j] ? dp[i + 1][j + 1] + 1 : Math.max(dp[i + 1][j], dp[i][j + 1]);
    }
  }
  const out: DiffLine[] = [];
  let i = 0;
  let j = 0;
  while (i < n && j < m) {
    if (a[i] === b[j]) {
      out.push({ kind: "keep", text: a[i] });
      i++;
      j++;
    } else if (dp[i + 1][j] >= dp[i][j + 1]) {
      out.push({ kind: "del", text: a[i++] });
    } else {
      out.push({ kind: "add", text: b[j++] });
    }
  }
  while (i < n) out.push({ kind: "del", text: a[i++] });
  while (j < m) out.push({ kind: "add", text: b[j++] });
  return out;
}

/** Esconde blocos longos de contexto, deixando `CONTEXT` linhas ao redor. */
export function withElision(lines: DiffLine[]): DiffLine[] {
  const keep = new Set<number>();
  lines.forEach((l, i) => {
    if (l.kind === "keep") return;
    for (let k = i - CONTEXT; k <= i + CONTEXT; k++) keep.add(k);
  });
  const out: DiffLine[] = [];
  let pulando = 0;
  const fecharLacuna = () => {
    if (pulando > 0) {
      out.push({ kind: "gap", text: `⋯ ${pulando} linha(s)` });
      pulando = 0;
    }
  };
  lines.forEach((l, i) => {
    if (keep.has(i)) {
      fecharLacuna();
      out.push(l);
    } else {
      pulando++;
    }
  });
  fecharLacuna();
  return out;
}

/** Diff pronto para desenhar. `oldText` nulo = arquivo novo. */
export function buildDiff(oldText: string | null, newText: string): DiffLine[] {
  const novo = newText.split("\n");
  if (oldText === null) return novo.map((text): DiffLine => ({ kind: "add", text }));
  return withElision(diffLines(oldText.split("\n"), novo));
}

export function diffStats(lines: DiffLine[]): { add: number; del: number } {
  let add = 0;
  let del = 0;
  for (const l of lines) {
    if (l.kind === "add") add++;
    if (l.kind === "del") del++;
  }
  return { add, del };
}
