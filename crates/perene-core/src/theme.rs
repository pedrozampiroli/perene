//! Temas: tokens da UI, paleta ANSI do terminal e cores de sintaxe do editor.
//!
//! Um tema descreve as três superfícies de uma vez, porque na prática o usuário
//! quer o app inteiro combinando — trocar só o editor deixa a UI destoando.
//!
//! Além dos embutidos, aceitamos **temas do Zed** (`from_zed_json`): são JSON
//! público (schema `themes/v0.2.0`) e um arquivo traz uma *família* com vários
//! temas dentro. As gramáticas do Zed não dá pra reaproveitar (tree-sitter
//! nativo), mas as cores sim.

use serde::{Deserialize, Serialize};

/// Cores da interface. Todo token vira uma CSS var (`--bg`, `--fg`, …) no front.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiColors {
    /// Fundo da janela.
    pub bg: String,
    /// Texto principal.
    pub fg: String,
    /// Fundo de painéis (sidebar, barras, modais).
    pub panel: String,
    /// Fundo levemente elevado (hover, linha selecionada).
    pub elevated: String,
    /// Bordas e divisores.
    pub border: String,
    /// Texto secundário.
    pub muted: String,
    /// Cor de destaque (foco, seleção ativa, links).
    pub accent: String,
    /// Texto sobre `accent`.
    pub accent_fg: String,
    /// Erros e ações destrutivas.
    pub danger: String,
    /// Avisos.
    pub warning: String,
    /// Sucesso / adições no diff.
    pub success: String,
    /// Fundo de seleção de texto.
    pub selection: String,
}

/// Paleta do terminal (xterm.js). Nomes iguais aos do xterm pra ir direto.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalColors {
    pub background: String,
    pub foreground: String,
    pub cursor: String,
    pub selection_background: String,
    pub black: String,
    pub red: String,
    pub green: String,
    pub yellow: String,
    pub blue: String,
    pub magenta: String,
    pub cyan: String,
    pub white: String,
    pub bright_black: String,
    pub bright_red: String,
    pub bright_green: String,
    pub bright_yellow: String,
    pub bright_blue: String,
    pub bright_magenta: String,
    pub bright_cyan: String,
    pub bright_white: String,
}

/// Cores de sintaxe do editor. Os nomes seguem os *captures* do Zed/tree-sitter,
/// justamente pra importação de tema do Zed ser uma tradução direta.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyntaxColors {
    pub comment: String,
    pub keyword: String,
    pub string: String,
    pub number: String,
    pub function: String,
    pub type_name: String,
    pub variable: String,
    pub constant: String,
    pub operator: String,
    pub punctuation: String,
    pub property: String,
    pub tag: String,
}

/// Um tema completo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Theme {
    /// Identificador estável (usado em `Settings.theme`).
    pub id: String,
    /// Nome exibido.
    pub name: String,
    /// `true` se for tema claro — o front usa pra `color-scheme`.
    pub light: bool,
    pub ui: UiColors,
    pub terminal: TerminalColors,
    pub syntax: SyntaxColors,
}

impl Theme {
    /// Tema padrão: o visual atual do app (Dark+ do VS Code), pra quem já usa
    /// não ver nada mudar ao atualizar.
    pub fn dark_plus() -> Self {
        Self {
            id: "dark-plus".into(),
            name: "Dark+".into(),
            light: false,
            ui: UiColors {
                bg: "#1e1e1e".into(),
                fg: "#d4d4d4".into(),
                panel: "#252526".into(),
                elevated: "#2d2d30".into(),
                border: "#3c3c3c".into(),
                muted: "#9aa0a6".into(),
                accent: "#0a84ff".into(),
                accent_fg: "#ffffff".into(),
                danger: "#f14c4c".into(),
                warning: "#e5e510".into(),
                success: "#23d18b".into(),
                selection: "#264f78".into(),
            },
            terminal: TerminalColors {
                background: "#1e1e1e".into(),
                foreground: "#d4d4d4".into(),
                cursor: "#d4d4d4".into(),
                selection_background: "#264f78".into(),
                black: "#000000".into(),
                red: "#cd3131".into(),
                green: "#0dbc79".into(),
                yellow: "#e5e510".into(),
                blue: "#2472c8".into(),
                magenta: "#bc3fbc".into(),
                cyan: "#11a8cd".into(),
                white: "#e5e5e5".into(),
                bright_black: "#666666".into(),
                bright_red: "#f14c4c".into(),
                bright_green: "#23d18b".into(),
                bright_yellow: "#f5f543".into(),
                bright_blue: "#3b8eea".into(),
                bright_magenta: "#d670d6".into(),
                bright_cyan: "#29b8db".into(),
                bright_white: "#ffffff".into(),
            },
            syntax: SyntaxColors {
                comment: "#6a9955".into(),
                keyword: "#569cd6".into(),
                string: "#ce9178".into(),
                number: "#b5cea8".into(),
                function: "#dcdcaa".into(),
                type_name: "#4ec9b0".into(),
                variable: "#9cdcfe".into(),
                constant: "#4fc1ff".into(),
                operator: "#d4d4d4".into(),
                punctuation: "#808080".into(),
                property: "#9cdcfe".into(),
                tag: "#569cd6".into(),
            },
        }
    }

    /// One Dark — o tema que o editor usava fixo antes (`@codemirror/theme-one-dark`).
    pub fn one_dark() -> Self {
        Self {
            id: "one-dark".into(),
            name: "One Dark".into(),
            light: false,
            ui: UiColors {
                bg: "#282c34".into(),
                fg: "#abb2bf".into(),
                panel: "#21252b".into(),
                elevated: "#2c313a".into(),
                border: "#3e4451".into(),
                muted: "#7f848e".into(),
                accent: "#61afef".into(),
                accent_fg: "#282c34".into(),
                danger: "#e06c75".into(),
                warning: "#e5c07b".into(),
                success: "#98c379".into(),
                selection: "#3e4451".into(),
            },
            terminal: TerminalColors {
                background: "#282c34".into(),
                foreground: "#abb2bf".into(),
                cursor: "#528bff".into(),
                selection_background: "#3e4451".into(),
                black: "#282c34".into(),
                red: "#e06c75".into(),
                green: "#98c379".into(),
                yellow: "#e5c07b".into(),
                blue: "#61afef".into(),
                magenta: "#c678dd".into(),
                cyan: "#56b6c2".into(),
                white: "#abb2bf".into(),
                bright_black: "#5c6370".into(),
                bright_red: "#e06c75".into(),
                bright_green: "#98c379".into(),
                bright_yellow: "#e5c07b".into(),
                bright_blue: "#61afef".into(),
                bright_magenta: "#c678dd".into(),
                bright_cyan: "#56b6c2".into(),
                bright_white: "#ffffff".into(),
            },
            syntax: SyntaxColors {
                comment: "#5c6370".into(),
                keyword: "#c678dd".into(),
                string: "#98c379".into(),
                number: "#d19a66".into(),
                function: "#61afef".into(),
                type_name: "#e5c07b".into(),
                variable: "#e06c75".into(),
                constant: "#d19a66".into(),
                operator: "#56b6c2".into(),
                punctuation: "#abb2bf".into(),
                property: "#e06c75".into(),
                tag: "#e06c75".into(),
            },
        }
    }

    /// Um tema claro, pra quem trabalha no sol.
    pub fn light() -> Self {
        Self {
            id: "light".into(),
            name: "Light".into(),
            light: true,
            ui: UiColors {
                bg: "#ffffff".into(),
                fg: "#24292f".into(),
                panel: "#f6f8fa".into(),
                elevated: "#eaeef2".into(),
                border: "#d0d7de".into(),
                muted: "#57606a".into(),
                accent: "#0969da".into(),
                accent_fg: "#ffffff".into(),
                danger: "#cf222e".into(),
                warning: "#9a6700".into(),
                success: "#1a7f37".into(),
                selection: "#b6d7ff".into(),
            },
            terminal: TerminalColors {
                background: "#ffffff".into(),
                foreground: "#24292f".into(),
                cursor: "#24292f".into(),
                selection_background: "#b6d7ff".into(),
                black: "#24292f".into(),
                red: "#cf222e".into(),
                green: "#1a7f37".into(),
                yellow: "#9a6700".into(),
                blue: "#0969da".into(),
                magenta: "#8250df".into(),
                cyan: "#1b7c83".into(),
                white: "#6e7781".into(),
                bright_black: "#57606a".into(),
                bright_red: "#a40e26".into(),
                bright_green: "#116329".into(),
                bright_yellow: "#7d4e00".into(),
                bright_blue: "#0550ae".into(),
                bright_magenta: "#6639ba".into(),
                bright_cyan: "#1b7c83".into(),
                bright_white: "#8c959f".into(),
            },
            syntax: SyntaxColors {
                comment: "#6e7781".into(),
                keyword: "#cf222e".into(),
                string: "#0a3069".into(),
                number: "#0550ae".into(),
                function: "#8250df".into(),
                type_name: "#953800".into(),
                variable: "#24292f".into(),
                constant: "#0550ae".into(),
                operator: "#cf222e".into(),
                punctuation: "#24292f".into(),
                property: "#0550ae".into(),
                tag: "#116329".into(),
            },
        }
    }

    /// Os temas que acompanham o app.
    pub fn builtins() -> Vec<Theme> {
        vec![Self::dark_plus(), Self::one_dark(), Self::light()]
    }

    /// Busca um tema embutido por id.
    pub fn builtin(id: &str) -> Option<Theme> {
        Self::builtins().into_iter().find(|t| t.id == id)
    }

    /// Traduz uma *família* de temas do Zed (`themes/v0.2.0`) para os nossos.
    ///
    /// Um arquivo do Zed traz várias variantes (`themes: [...]`), então isto
    /// devolve uma lista. Campos ausentes caem no tema embutido de mesma
    /// luminosidade — tema do Zed não é obrigado a definir tudo, e faltar uma
    /// cor não pode deixar um pedaço da UI invisível.
    pub fn from_zed_json(raw: &str) -> Result<Vec<Theme>, String> {
        let family: ZedFamily = serde_json::from_str(raw).map_err(|e| e.to_string())?;
        if family.themes.is_empty() {
            return Err("arquivo de tema do Zed sem nenhum tema dentro".into());
        }
        Ok(family
            .themes
            .into_iter()
            .map(|z| Theme::from_zed_variant(&family.name, z))
            .collect())
    }

    fn from_zed_variant(family_name: &str, z: ZedTheme) -> Theme {
        let light = z.appearance.eq_ignore_ascii_case("light");
        let base = if light { Theme::light() } else { Theme::dark_plus() };
        let s = &z.style;

        // O Zed guarda a sintaxe como mapa de capture → {color, font_style}.
        let syn = |key: &str, fallback: &str| -> String {
            s.syntax
                .get(key)
                .and_then(|v| v.color.clone())
                .map(|c| normalize_hex(&c))
                .unwrap_or_else(|| fallback.to_string())
        };
        let pick = |v: &Option<String>, fallback: &str| -> String {
            v.as_ref()
                .map(|c| normalize_hex(c))
                .unwrap_or_else(|| fallback.to_string())
        };

        let id = format!(
            "zed-{}",
            slugify(&format!("{} {}", family_name, z.name)).trim_matches('-')
        );

        Theme {
            id,
            name: z.name.clone(),
            light,
            ui: UiColors {
                bg: pick(&s.background, &base.ui.bg),
                fg: pick(&s.text, &base.ui.fg),
                panel: pick(&s.surface_background, &base.ui.panel),
                elevated: pick(&s.elevated_surface_background, &base.ui.elevated),
                border: pick(&s.border, &base.ui.border),
                muted: pick(&s.text_muted, &base.ui.muted),
                accent: pick(&s.text_accent, &base.ui.accent),
                accent_fg: base.ui.accent_fg.clone(),
                danger: pick(&s.error, &base.ui.danger),
                warning: pick(&s.warning, &base.ui.warning),
                success: pick(&s.created, &base.ui.success),
                selection: pick(&s.element_selected, &base.ui.selection),
            },
            terminal: TerminalColors {
                background: pick(&s.terminal_background, &pick(&s.background, &base.terminal.background)),
                foreground: pick(&s.terminal_foreground, &pick(&s.text, &base.terminal.foreground)),
                cursor: pick(&s.text, &base.terminal.cursor),
                selection_background: pick(&s.element_selected, &base.terminal.selection_background),
                black: pick(&s.terminal_ansi_black, &base.terminal.black),
                red: pick(&s.terminal_ansi_red, &base.terminal.red),
                green: pick(&s.terminal_ansi_green, &base.terminal.green),
                yellow: pick(&s.terminal_ansi_yellow, &base.terminal.yellow),
                blue: pick(&s.terminal_ansi_blue, &base.terminal.blue),
                magenta: pick(&s.terminal_ansi_magenta, &base.terminal.magenta),
                cyan: pick(&s.terminal_ansi_cyan, &base.terminal.cyan),
                white: pick(&s.terminal_ansi_white, &base.terminal.white),
                bright_black: pick(&s.terminal_ansi_bright_black, &base.terminal.bright_black),
                bright_red: pick(&s.terminal_ansi_bright_red, &base.terminal.bright_red),
                bright_green: pick(&s.terminal_ansi_bright_green, &base.terminal.bright_green),
                bright_yellow: pick(&s.terminal_ansi_bright_yellow, &base.terminal.bright_yellow),
                bright_blue: pick(&s.terminal_ansi_bright_blue, &base.terminal.bright_blue),
                bright_magenta: pick(&s.terminal_ansi_bright_magenta, &base.terminal.bright_magenta),
                bright_cyan: pick(&s.terminal_ansi_bright_cyan, &base.terminal.bright_cyan),
                bright_white: pick(&s.terminal_ansi_bright_white, &base.terminal.bright_white),
            },
            syntax: SyntaxColors {
                comment: syn("comment", &base.syntax.comment),
                keyword: syn("keyword", &base.syntax.keyword),
                string: syn("string", &base.syntax.string),
                number: syn("number", &base.syntax.number),
                function: syn("function", &base.syntax.function),
                type_name: syn("type", &base.syntax.type_name),
                variable: syn("variable", &base.syntax.variable),
                constant: syn("constant", &base.syntax.constant),
                operator: syn("operator", &base.syntax.operator),
                punctuation: syn("punctuation", &base.syntax.punctuation),
                property: syn("property", &base.syntax.property),
                tag: syn("tag", &base.syntax.tag),
            },
        }
    }
}

/// O Zed escreve cores como `#rrggbbaa`. O canal alfa não serve pra nós (as
/// superfícies são opacas) e o CSS aceitaria, mas o xterm.js não — corta em 7.
fn normalize_hex(c: &str) -> String {
    let c = c.trim();
    if c.len() == 9 && c.starts_with('#') {
        c[..7].to_string()
    } else {
        c.to_string()
    }
}

fn slugify(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut last_dash = false;
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Espelho do JSON do Zed. Tudo opcional: temas de terceiros definem subconjuntos
// diferentes, e um campo faltando não pode virar erro de parse.
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct ZedFamily {
    #[serde(default)]
    name: String,
    #[serde(default)]
    themes: Vec<ZedTheme>,
}

#[derive(Debug, Deserialize)]
struct ZedTheme {
    #[serde(default)]
    name: String,
    #[serde(default)]
    appearance: String,
    #[serde(default)]
    style: ZedStyle,
}

#[derive(Debug, Default, Deserialize)]
struct ZedStyle {
    background: Option<String>,
    border: Option<String>,
    text: Option<String>,
    #[serde(rename = "text.muted")]
    text_muted: Option<String>,
    #[serde(rename = "text.accent")]
    text_accent: Option<String>,
    #[serde(rename = "surface.background")]
    surface_background: Option<String>,
    #[serde(rename = "elevated_surface.background")]
    elevated_surface_background: Option<String>,
    #[serde(rename = "element.selected")]
    element_selected: Option<String>,
    error: Option<String>,
    warning: Option<String>,
    created: Option<String>,
    #[serde(rename = "terminal.background")]
    terminal_background: Option<String>,
    #[serde(rename = "terminal.foreground")]
    terminal_foreground: Option<String>,
    #[serde(rename = "terminal.ansi.black")]
    terminal_ansi_black: Option<String>,
    #[serde(rename = "terminal.ansi.red")]
    terminal_ansi_red: Option<String>,
    #[serde(rename = "terminal.ansi.green")]
    terminal_ansi_green: Option<String>,
    #[serde(rename = "terminal.ansi.yellow")]
    terminal_ansi_yellow: Option<String>,
    #[serde(rename = "terminal.ansi.blue")]
    terminal_ansi_blue: Option<String>,
    #[serde(rename = "terminal.ansi.magenta")]
    terminal_ansi_magenta: Option<String>,
    #[serde(rename = "terminal.ansi.cyan")]
    terminal_ansi_cyan: Option<String>,
    #[serde(rename = "terminal.ansi.white")]
    terminal_ansi_white: Option<String>,
    #[serde(rename = "terminal.ansi.bright_black")]
    terminal_ansi_bright_black: Option<String>,
    #[serde(rename = "terminal.ansi.bright_red")]
    terminal_ansi_bright_red: Option<String>,
    #[serde(rename = "terminal.ansi.bright_green")]
    terminal_ansi_bright_green: Option<String>,
    #[serde(rename = "terminal.ansi.bright_yellow")]
    terminal_ansi_bright_yellow: Option<String>,
    #[serde(rename = "terminal.ansi.bright_blue")]
    terminal_ansi_bright_blue: Option<String>,
    #[serde(rename = "terminal.ansi.bright_magenta")]
    terminal_ansi_bright_magenta: Option<String>,
    #[serde(rename = "terminal.ansi.bright_cyan")]
    terminal_ansi_bright_cyan: Option<String>,
    #[serde(rename = "terminal.ansi.bright_white")]
    terminal_ansi_bright_white: Option<String>,
    #[serde(default)]
    syntax: std::collections::HashMap<String, ZedSyntaxStyle>,
}

#[derive(Debug, Deserialize)]
struct ZedSyntaxStyle {
    #[serde(default)]
    color: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_have_unique_ids() {
        let ids: Vec<_> = Theme::builtins().into_iter().map(|t| t.id).collect();
        let mut sorted = ids.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(ids.len(), sorted.len(), "ids de tema embutido duplicados");
    }

    #[test]
    fn builtin_lookup_works() {
        assert_eq!(Theme::builtin("one-dark").unwrap().name, "One Dark");
        assert!(Theme::builtin("nao-existe").is_none());
    }

    #[test]
    fn zed_family_becomes_one_theme_per_variant() {
        let raw = r##"{
          "name": "Ayu",
          "themes": [
            {"name": "Ayu Dark", "appearance": "dark", "style": {
               "background": "#0b0e14ff", "text": "#bfbdb6ff",
               "terminal.ansi.red": "#ff6666ff",
               "syntax": {"keyword": {"color": "#ff8f40ff"}, "string": {"color": "#aad94cff"}}
            }},
            {"name": "Ayu Light", "appearance": "light", "style": {"background": "#fcfcfcff"}}
          ]
        }"##;
        let themes = Theme::from_zed_json(raw).unwrap();
        assert_eq!(themes.len(), 2);

        let dark = &themes[0];
        assert_eq!(dark.name, "Ayu Dark");
        assert!(!dark.light);
        assert_eq!(dark.id, "zed-ayu-ayu-dark");
        // alfa cortado
        assert_eq!(dark.ui.bg, "#0b0e14");
        assert_eq!(dark.terminal.red, "#ff6666");
        assert_eq!(dark.syntax.keyword, "#ff8f40");

        let light = &themes[1];
        assert!(light.light);
        assert_eq!(light.ui.bg, "#fcfcfc");
        // o que o tema não definiu cai no embutido claro, nunca em vazio
        assert_eq!(light.ui.fg, Theme::light().ui.fg);
        assert!(!light.syntax.comment.is_empty());
    }

    #[test]
    fn zed_json_without_themes_is_an_error() {
        assert!(Theme::from_zed_json(r##"{"name":"X","themes":[]}"##).is_err());
        assert!(Theme::from_zed_json("nao e json").is_err());
    }

    #[test]
    fn unknown_zed_fields_do_not_break_parsing() {
        // Temas reais trazem dezenas de chaves que não usamos.
        let raw = r##"{"name":"F","themes":[{"name":"T","appearance":"dark",
          "style":{"scrollbar.thumb.background":"#fff","players":[{"cursor":"#abc"}],
                   "background":"#111111ff"}}]}"##;
        let t = &Theme::from_zed_json(raw).unwrap()[0];
        assert_eq!(t.ui.bg, "#111111");
    }
}
