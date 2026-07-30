// Helpers de caminho para RÓTULOS da UI.
//
// No Windows o backend devolve caminhos com `\` (e às vezes misturados, quando
// a raiz vem do usuário e o resto é montado por nós). Splitar só em "/" fazia o
// rótulo virar o caminho inteiro — a aba de um projeto aparecia como
// "C:\Users\fulano\proj" em vez de "proj". Todo split de caminho na UI passa por
// aqui.

/** Separador de caminho: `/` ou `\`, um ou mais. */
const SEP = /[\\/]+/;

/** Segmentos não vazios do caminho. */
export function pathParts(path: string): string[] {
  return path.split(SEP).filter(Boolean);
}

/** Último segmento: `C:\a\b` → `b`, `/a/b/` → `b`. Vazio se não houver. */
export function baseName(path: string): string {
  const parts = pathParts(path);
  return parts[parts.length - 1] ?? "";
}

/** Caminho encurtado para rótulo: `…/penúltimo/último`, no separador nativo. */
export function shortPath(path: string): string {
  const sep = path.includes("\\") ? "\\" : "/";
  const parts = pathParts(path);
  if (parts.length <= 2) {
    return (path.startsWith("/") ? "/" : "") + parts.join(sep);
  }
  return "…" + sep + parts.slice(-2).join(sep);
}

/** O caminho está dentro de `.perene/worktrees/`? (aceita os dois separadores) */
export function isInWorktree(path: string): boolean {
  return /[\\/]\.perene[\\/]worktrees[\\/]/.test(path);
}
