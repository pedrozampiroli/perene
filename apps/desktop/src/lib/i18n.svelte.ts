// i18n baseado em arquivos JSON.
//
// Para adicionar um idioma: **duplique** `src/i18n/en.json`, traduza os valores
// e salve como `<code>.json` (ex.: `fr.json`, `de.json`). Ele é detectado
// automaticamente (import.meta.glob) e aparece no seletor das configurações —
// nenhum código precisa mudar. O `$meta.name` é o rótulo mostrado ao usuário.
//
// O inglês (`en`) é a fonte da verdade: chaves faltando em outro idioma caem
// automaticamente no texto em inglês.

type Dict = Record<string, string | { name?: string; flag?: string }>;

const FILES = import.meta.glob<{ default: Dict }>("../i18n/*.json", { eager: true });

/** { "en": {...}, "pt-BR": {...} } */
const DICTS: Record<string, Dict> = {};
for (const [path, mod] of Object.entries(FILES)) {
  const code = path.split("/").pop()!.replace(".json", "");
  DICTS[code] = mod.default;
}

const FALLBACK = "en";

export interface LocaleInfo {
  code: string;
  name: string;
  flag: string;
}

/** Idiomas disponíveis (derivados dos arquivos existentes). */
export const LOCALES: LocaleInfo[] = Object.entries(DICTS)
  .map(([code, d]) => {
    const meta = (d["$meta"] ?? {}) as { name?: string; flag?: string };
    return { code, name: meta.name ?? code, flag: meta.flag ?? "" };
  })
  .sort((a, b) => a.name.localeCompare(b.name));

/** Melhor idioma para o sistema do usuário (usado quando não há preferência). */
export function detectLocale(): string {
  const langs = navigator.languages?.length ? navigator.languages : [navigator.language];
  for (const l of langs) {
    if (DICTS[l]) return l; // match exato (ex.: pt-BR)
    const base = l.split("-")[0];
    const hit = Object.keys(DICTS).find((c) => c === base || c.split("-")[0] === base);
    if (hit) return hit;
  }
  return FALLBACK;
}

class I18n {
  /** Código do idioma ativo. `""` (vazio) nas settings = seguir o sistema. */
  locale = $state<string>(FALLBACK);

  setLocale(code: string): void {
    this.locale = DICTS[code] ? code : detectLocale();
  }

  /** Traduz `key`, interpolando `{placeholders}`. Sem tradução, devolve o inglês
   *  e, em último caso, a própria chave (fica óbvio o que falta traduzir). */
  t(key: string, params?: Record<string, string | number>): string {
    const dict = DICTS[this.locale] ?? DICTS[FALLBACK];
    let value = dict?.[key] ?? DICTS[FALLBACK]?.[key];
    if (typeof value !== "string") return key;
    if (params) {
      for (const [k, v] of Object.entries(params)) {
        value = (value as string).replaceAll(`{${k}}`, String(v));
      }
    }
    return value as string;
  }
}

export const i18n = new I18n();

/** Atalho: `t("some.key")`. */
export function t(key: string, params?: Record<string, string | number>): string {
  return i18n.t(key, params);
}
