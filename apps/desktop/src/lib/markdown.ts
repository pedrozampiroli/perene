// Markdown do agente → HTML seguro.
//
// A resposta do agente é conteúdo de terceiro rodando dentro do nosso webview:
// nunca vai pro DOM sem passar pelo sanitizador. `marked` converte, `DOMPurify`
// tira o que puder executar (script, on*, javascript:).
//
// Streaming: renderizamos a cada chunk, então o texto passa por aqui com
// markdown pela metade (fence aberto, link incompleto). O `marked` aguenta —
// o que não pode é o resultado piscar entre um estado e outro, por isso nada de
// heurística de "espera fechar".

import { marked } from "marked";
import DOMPurify from "dompurify";

marked.setOptions({
  gfm: true,
  breaks: true, // quebra de linha simples vira <br>, como todo chat
});

/** Links abrem fora do app; sem isso um clique substituiria a janela toda. */
DOMPurify.addHook("afterSanitizeAttributes", (node) => {
  if (node.tagName === "A") {
    node.setAttribute("target", "_blank");
    node.setAttribute("rel", "noopener noreferrer");
  }
});

export function renderMarkdown(source: string): string {
  const html = marked.parse(source, { async: false }) as string;
  return DOMPurify.sanitize(html, {
    // `data:` só para imagem (o usuário cola print e ele aparece no eco).
    ALLOWED_URI_REGEXP: /^(?:https?|mailto|data:image\/):/i,
  });
}
