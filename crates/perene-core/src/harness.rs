//! Configuração dos harnesses (claude / codex / opencode): servidores MCP e skills.
//!
//! Estes arquivos são **do usuário, não nossos**. `~/.claude.json` guarda muito
//! mais que MCP (onboarding, histórico por projeto, caches), então tudo aqui lê,
//! altera só a nossa chave e regrava preservando o resto — `serde_json::Value` e
//! `toml::Value` no meio do caminho, nunca structs fechadas.
//!
//! Todo caminho é injetado via [`HarnessPaths`]: os testes usam `tempfile` e
//! jamais encostam no estado real (regra 1 do projeto).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Qual ferramenta.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Harness {
    Claude,
    Codex,
    OpenCode,
}

impl Harness {
    pub fn id(&self) -> &'static str {
        match self {
            Harness::Claude => "claude",
            Harness::Codex => "codex",
            Harness::OpenCode => "opencode",
        }
    }

    pub fn all() -> [Harness; 3] {
        [Harness::Claude, Harness::Codex, Harness::OpenCode]
    }
}

/// Um servidor MCP, normalizado entre os três formatos.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServer {
    pub name: String,
    /// Executável (transporte stdio). Vazio quando é servidor remoto.
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// URL (transporte http/sse). Vazio quando é stdio.
    #[serde(default)]
    pub url: String,
    /// `false` quando o servidor está guardado por nós, fora do arquivo da
    /// ferramenta (ver [`HarnessStore::set_enabled`]).
    #[serde(default = "yes")]
    pub enabled: bool,
}

fn yes() -> bool {
    true
}

/// Uma skill (`SKILL.md` com frontmatter).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Skill {
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// Diretório da skill.
    pub path: String,
    /// `true` se está no diretório do projeto, `false` se global.
    pub project_scoped: bool,
    /// `true` quando veio de `.agents/skills` — o diretório COMPARTILHADO, lido
    /// por codex e opencode. A UI marca essas para o usuário saber que mexer
    /// ali afeta mais de uma ferramenta.
    #[serde(default)]
    pub shared: bool,
}

/// Onde cada arquivo mora. `at_home()` dá os caminhos reais; os testes injetam.
#[derive(Debug, Clone)]
pub struct HarnessPaths {
    pub claude_json: PathBuf,
    pub codex_toml: PathBuf,
    pub opencode_json: PathBuf,
    /// Raiz para montar os diretórios de skills globais (`$HOME`, ou o tempdir
    /// nos testes).
    pub skills_home: PathBuf,
    /// Onde guardamos os servidores desligados (dentro do NOSSO estado).
    pub disabled_store: PathBuf,
}

/// Subdiretório de skills **próprio** de cada ferramenta, relativo a uma raiz.
///
/// Confirmado nos binários instalados: o claude lê `.claude/skills`, o codex
/// `$CODEX_HOME/skills` (padrão `~/.codex/skills`) e o opencode
/// `.opencode/skills`.
fn own_skills_dir(h: Harness) -> &'static str {
    match h {
        Harness::Claude => ".claude",
        Harness::Codex => ".codex",
        Harness::OpenCode => ".opencode",
    }
}

/// Diretório de skills **compartilhado**, lido por codex e opencode.
///
/// É a convenção que permite uma skill servir as duas ferramentas sem cópia. O
/// claude não lê daqui (só `.claude/skills`).
const SHARED_SKILLS: &str = ".agents";

/// `true` se a ferramenta também lê o diretório compartilhado.
fn reads_shared_skills(h: Harness) -> bool {
    !matches!(h, Harness::Claude)
}

impl HarnessPaths {
    pub fn at_home() -> Self {
        let home = crate::paths::home_dir();
        Self {
            claude_json: home.join(".claude.json"),
            codex_toml: home.join(".codex").join("config.toml"),
            opencode_json: home.join(".config").join("opencode").join("opencode.json"),
            skills_home: home.clone(),
            disabled_store: crate::paths::state_dir().join("harness-disabled.json"),
        }
    }

    pub fn config_for(&self, h: Harness) -> &Path {
        match h {
            Harness::Claude => &self.claude_json,
            Harness::Codex => &self.codex_toml,
            Harness::OpenCode => &self.opencode_json,
        }
    }
}

pub struct HarnessStore {
    paths: HarnessPaths,
}

/// Servidores desligados, por harness. Guardamos a config inteira pra religar
/// sem perder nada — desligar não pode custar o `env` que o usuário digitou.
type DisabledMap = BTreeMap<String, BTreeMap<String, McpServer>>;

impl HarnessStore {
    pub fn new(paths: HarnessPaths) -> Self {
        Self { paths }
    }

    pub fn at_home() -> Self {
        Self::new(HarnessPaths::at_home())
    }

    pub fn paths(&self) -> &HarnessPaths {
        &self.paths
    }

    /// `true` se a ferramenta tem arquivo de config no disco. A UI usa pra não
    /// oferecer configuração de uma CLI que o usuário nem instalou.
    pub fn is_configured(&self, h: Harness) -> bool {
        self.paths.config_for(h).exists()
    }

    // -- MCP ---------------------------------------------------------------

    /// Servidores de um harness: os ativos (lidos do arquivo da ferramenta) e os
    /// desligados (guardados por nós), em ordem alfabética.
    pub fn list_mcp(&self, h: Harness) -> Result<Vec<McpServer>, String> {
        let mut out = match h {
            Harness::Claude => self.read_claude()?,
            Harness::Codex => self.read_codex()?,
            Harness::OpenCode => self.read_opencode()?,
        };
        for (_, mut s) in self.disabled_for(h) {
            s.enabled = false;
            out.push(s);
        }
        out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        Ok(out)
    }

    /// Cria ou atualiza um servidor no arquivo da ferramenta.
    pub fn upsert_mcp(&self, h: Harness, server: &McpServer) -> Result<(), String> {
        if server.name.trim().is_empty() {
            return Err("servidor MCP precisa de nome".into());
        }
        if server.command.trim().is_empty() && server.url.trim().is_empty() {
            return Err("servidor MCP precisa de um comando ou de uma URL".into());
        }
        // Se estava desligado, sai da nossa lista: a fonte da verdade volta a ser
        // o arquivo da ferramenta.
        self.forget_disabled(h, &server.name)?;
        match h {
            Harness::Claude => self.write_claude(server, false),
            Harness::Codex => self.write_codex(server, false),
            Harness::OpenCode => self.write_opencode(server, false),
        }
    }

    /// Remove de vez (do arquivo da ferramenta e da nossa lista de desligados).
    pub fn remove_mcp(&self, h: Harness, name: &str) -> Result<(), String> {
        self.forget_disabled(h, name)?;
        let stub = McpServer {
            name: name.to_string(),
            command: String::new(),
            args: vec![],
            env: BTreeMap::new(),
            url: String::new(),
            enabled: true,
        };
        match h {
            Harness::Claude => self.write_claude(&stub, true),
            Harness::Codex => self.write_codex(&stub, true),
            Harness::OpenCode => self.write_opencode(&stub, true),
        }
    }

    /// Liga/desliga sem perder configuração: desligar tira do arquivo da
    /// ferramenta e guarda no nosso; ligar faz o caminho de volta.
    pub fn set_enabled(&self, h: Harness, name: &str, enabled: bool) -> Result<(), String> {
        if enabled {
            let mut disabled = self.load_disabled()?;
            let server = disabled
                .get_mut(h.id())
                .and_then(|m| m.remove(name))
                .ok_or_else(|| format!("servidor '{name}' não está desligado"))?;
            self.save_disabled(&disabled)?;
            match h {
                Harness::Claude => self.write_claude(&server, false),
                Harness::Codex => self.write_codex(&server, false),
                Harness::OpenCode => self.write_opencode(&server, false),
            }
        } else {
            let server = self
                .list_mcp(h)?
                .into_iter()
                .find(|s| s.name == name)
                .ok_or_else(|| format!("servidor '{name}' não encontrado"))?;
            let mut disabled = self.load_disabled()?;
            disabled
                .entry(h.id().to_string())
                .or_default()
                .insert(name.to_string(), server.clone());
            self.save_disabled(&disabled)?;
            match h {
                Harness::Claude => self.write_claude(&server, true),
                Harness::Codex => self.write_codex(&server, true),
                Harness::OpenCode => self.write_opencode(&server, true),
            }
        }
    }

    // -- leitura por formato ------------------------------------------------

    fn read_json(path: &Path) -> Result<serde_json::Value, String> {
        match std::fs::read_to_string(path) {
            Ok(raw) => serde_json::from_str(&raw).map_err(|e| format!("{}: {e}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Ok(serde_json::Value::Object(Default::default()))
            }
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }

    fn read_claude(&self) -> Result<Vec<McpServer>, String> {
        let root = Self::read_json(&self.paths.claude_json)?;
        Ok(root
            .get("mcpServers")
            .and_then(|v| v.as_object())
            .map(|m| m.iter().map(|(k, v)| server_from_json(k, v)).collect())
            .unwrap_or_default())
    }

    fn read_opencode(&self) -> Result<Vec<McpServer>, String> {
        let root = Self::read_json(&self.paths.opencode_json)?;
        Ok(root
            .get("mcp")
            .and_then(|v| v.as_object())
            .map(|m| m.iter().map(|(k, v)| server_from_json(k, v)).collect())
            .unwrap_or_default())
    }

    fn read_codex(&self) -> Result<Vec<McpServer>, String> {
        let raw = match std::fs::read_to_string(&self.paths.codex_toml) {
            Ok(r) => r,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
            Err(e) => return Err(format!("{}: {e}", self.paths.codex_toml.display())),
        };
        let doc: toml::Value = toml::from_str(&raw)
            .map_err(|e| format!("{}: {e}", self.paths.codex_toml.display()))?;
        let table = match doc.get("mcp_servers").and_then(|v| v.as_table()) {
            Some(t) => t,
            None => return Ok(vec![]),
        };
        Ok(table
            .iter()
            .map(|(name, v)| McpServer {
                name: name.clone(),
                command: v
                    .get("command")
                    .and_then(|c| c.as_str())
                    .unwrap_or_default()
                    .to_string(),
                args: v
                    .get("args")
                    .and_then(|a| a.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|x| x.as_str().map(str::to_string))
                            .collect()
                    })
                    .unwrap_or_default(),
                env: v
                    .get("env")
                    .and_then(|e| e.as_table())
                    .map(|t| {
                        t.iter()
                            .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                            .collect()
                    })
                    .unwrap_or_default(),
                url: v
                    .get("url")
                    .and_then(|u| u.as_str())
                    .unwrap_or_default()
                    .to_string(),
                enabled: true,
            })
            .collect())
    }

    // -- escrita por formato ------------------------------------------------
    //
    // `remove = true` apaga a entrada; caso contrário insere/atualiza. Em todos
    // os casos, o resto do arquivo do usuário é preservado byte a byte pelo
    // round-trip de Value.

    /// Reescreve **só** a chave de MCP, deixando o resto do arquivo como texto.
    ///
    /// O truque é o `RawValue`: o nível de cima é lido como `nome → texto cru`,
    /// então tudo que não é nosso volta pro disco byte a byte. Sem isso, o
    /// round-trip por `Value` mexia no arquivo do usuário de dois jeitos:
    /// alfabetizava as chaves e alterava floats longos em 1 ULP
    /// (`0.40265999999974156` → `0.4026599999997416`) — porque f64 não
    /// representa aquele texto exatamente, e o serde regrava o que coube.
    fn write_json_key(
        path: &Path,
        key: &str,
        server: &McpServer,
        remove: bool,
        to_value: fn(&McpServer) -> serde_json::Value,
    ) -> Result<(), String> {
        use serde_json::value::RawValue;

        // IndexMap, não serde_json::Map: este último só existe para `Value`, e
        // aqui o valor é texto cru. Ordem de inserção = ordem do arquivo.
        type RawMap = indexmap::IndexMap<String, Box<RawValue>>;

        let mut root: RawMap = match std::fs::read_to_string(path) {
            Ok(raw) if !raw.trim().is_empty() => {
                serde_json::from_str(&raw).map_err(|e| format!("{}: {e}", path.display()))?
            }
            Ok(_) => RawMap::new(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => RawMap::new(),
            Err(e) => return Err(format!("{}: {e}", path.display())),
        };

        // Só a nossa chave é desserializada de verdade.
        let mut servers: serde_json::Map<String, serde_json::Value> = match root.get(key) {
            Some(raw) => serde_json::from_str(raw.get())
                .map_err(|_| format!("{}: '{key}' não é um objeto", path.display()))?,
            None => Default::default(),
        };

        if remove {
            servers.remove(&server.name);
        } else {
            servers.insert(server.name.clone(), to_value(server));
        }

        if servers.is_empty() {
            // Ausente e vazio significam a mesma coisa para as três CLIs, e
            // deixar `"mcpServers": {}` num arquivo que não tinha a chave é
            // sujeira nossa em arquivo alheio.
            //
            // `shift_remove`, não `remove`: o segundo é swap-remove e jogaria a
            // última chave do arquivo para o buraco — reordenando exatamente o
            // que este método existe para preservar.
            root.shift_remove(key);
        } else {
            let encoded = serde_json::to_string(&serde_json::Value::Object(servers))
                .map_err(|e| e.to_string())?;
            let raw = RawValue::from_string(encoded).map_err(|e| e.to_string())?;
            root.insert(key.to_string(), raw);
        }

        crate::store::write_json_atomic(path, &root).map_err(|e| e.to_string())
    }

    fn write_claude(&self, server: &McpServer, remove: bool) -> Result<(), String> {
        Self::write_json_key(
            &self.paths.claude_json,
            "mcpServers",
            server,
            remove,
            claude_value,
        )
    }

    fn write_opencode(&self, server: &McpServer, remove: bool) -> Result<(), String> {
        Self::write_json_key(
            &self.paths.opencode_json,
            "mcp",
            server,
            remove,
            opencode_value,
        )
    }

    fn write_codex(&self, server: &McpServer, remove: bool) -> Result<(), String> {
        let path = &self.paths.codex_toml;
        let raw = std::fs::read_to_string(path).unwrap_or_default();
        let mut doc: toml::Value = if raw.trim().is_empty() {
            toml::Value::Table(Default::default())
        } else {
            toml::from_str(&raw).map_err(|e| format!("{}: {e}", path.display()))?
        };

        {
            let root = doc
                .as_table_mut()
                .ok_or_else(|| format!("{}: raiz não é uma tabela TOML", path.display()))?;
            let servers = root
                .entry("mcp_servers".to_string())
                .or_insert_with(|| toml::Value::Table(Default::default()))
                .as_table_mut()
                .ok_or_else(|| format!("{}: 'mcp_servers' não é uma tabela", path.display()))?;
            if remove {
                servers.remove(&server.name);
            } else {
                servers.insert(server.name.clone(), codex_value(server));
            }
            // Mesma regra do JSON: seção vazia é igual a seção ausente, e não
            // deixamos `[mcp_servers]` de lembrança no arquivo do usuário.
            let ficou_vazia = servers.is_empty();
            if ficou_vazia {
                root.remove("mcp_servers");
            }
        }

        let text = toml::to_string_pretty(&doc).map_err(|e| e.to_string())?;
        write_text_atomic(path, &text)
    }

    // -- servidores desligados (nosso estado) --------------------------------

    fn load_disabled(&self) -> Result<DisabledMap, String> {
        match std::fs::read_to_string(&self.paths.disabled_store) {
            Ok(raw) => serde_json::from_str(&raw).map_err(|e| e.to_string()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(DisabledMap::new()),
            Err(e) => Err(e.to_string()),
        }
    }

    fn save_disabled(&self, map: &DisabledMap) -> Result<(), String> {
        crate::store::write_json_atomic(&self.paths.disabled_store, map).map_err(|e| e.to_string())
    }

    fn disabled_for(&self, h: Harness) -> BTreeMap<String, McpServer> {
        self.load_disabled()
            .unwrap_or_default()
            .get(h.id())
            .cloned()
            .unwrap_or_default()
    }

    fn forget_disabled(&self, h: Harness, name: &str) -> Result<(), String> {
        let mut disabled = self.load_disabled()?;
        if let Some(m) = disabled.get_mut(h.id()) {
            if m.remove(name).is_some() {
                return self.save_disabled(&disabled);
            }
        }
        Ok(())
    }

    // -- skills ---------------------------------------------------------------

    /// Diretórios que uma ferramenta lê, com a origem de cada um.
    ///
    /// Cada ferramenta tem o seu (`.claude/skills`, `.codex/skills`,
    /// `.opencode/skills`); codex e opencode leem **também** o compartilhado
    /// `.agents/skills`, que é como uma skill serve as duas sem cópia.
    fn skills_dirs(&self, h: Harness, project: Option<&Path>) -> Vec<(PathBuf, bool, bool)> {
        // (caminho, é do projeto, é o compartilhado)
        let mut raizes: Vec<(PathBuf, bool)> = vec![(self.paths.skills_home.clone(), false)];
        if let Some(p) = project {
            raizes.push((p.to_path_buf(), true));
        }
        let mut out = Vec::new();
        for (raiz, do_projeto) in raizes {
            out.push((
                raiz.join(own_skills_dir(h)).join("skills"),
                do_projeto,
                false,
            ));
            if reads_shared_skills(h) {
                out.push((raiz.join(SHARED_SKILLS).join("skills"), do_projeto, true));
            }
        }
        out
    }

    /// Skills que a ferramenta enxerga: as globais e, com `project`, as do
    /// projeto.
    pub fn list_skills(&self, h: Harness, project: Option<&Path>) -> Vec<Skill> {
        let mut out = Vec::new();
        for (dir, do_projeto, compartilhado) in self.skills_dirs(h, project) {
            out.extend(read_skills_dir(&dir, do_projeto, compartilhado));
        }
        out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        out
    }

    /// Instala uma skill copiando um diretório que contenha `SKILL.md`.
    ///
    /// Para codex e opencode o destino é o **compartilhado** (`.agents/skills`):
    /// instalar uma vez e as duas enxergarem é o comportamento útil; quem quiser
    /// isolar move o diretório depois.
    pub fn install_skill(
        &self,
        h: Harness,
        source: &Path,
        project: Option<&Path>,
    ) -> Result<Skill, String> {
        if !source.join("SKILL.md").is_file() {
            return Err(format!("{} não tem SKILL.md", source.display()));
        }
        let name = source
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or("diretório de origem sem nome")?;
        let compartilhado = reads_shared_skills(h);
        let raiz = match project {
            Some(p) => p.to_path_buf(),
            None => self.paths.skills_home.clone(),
        };
        let dest_root = raiz
            .join(if compartilhado {
                SHARED_SKILLS
            } else {
                own_skills_dir(h)
            })
            .join("skills");
        let dest = dest_root.join(name);
        if dest.exists() {
            return Err(format!(
                "já existe uma skill '{name}' em {}",
                dest_root.display()
            ));
        }
        copy_dir(source, &dest).map_err(|e| e.to_string())?;
        read_skill(&dest, project.is_some(), compartilhado)
            .ok_or_else(|| "SKILL.md ilegível após copiar".into())
    }

    /// Remove uma skill instalada. Só apaga dentro dos diretórios de skills —
    /// um caminho de fora é recusado, pra um bug de UI não virar `rm -rf` na
    /// árvore do usuário.
    pub fn remove_skill(
        &self,
        h: Harness,
        path: &Path,
        project: Option<&Path>,
    ) -> Result<(), String> {
        let ok = self
            .skills_dirs(h, project)
            .iter()
            .any(|(root, _, _)| path.starts_with(root) && path != root);
        if !ok {
            return Err(format!(
                "{} está fora dos diretórios de skills",
                path.display()
            ));
        }
        std::fs::remove_dir_all(path).map_err(|e| e.to_string())
    }
}

// -- helpers de formato ------------------------------------------------------

fn server_from_json(name: &str, v: &serde_json::Value) -> McpServer {
    // `command` pode ser string (claude) ou array (opencode).
    let (command, mut args) = match v.get("command") {
        Some(serde_json::Value::String(s)) => (s.clone(), vec![]),
        Some(serde_json::Value::Array(a)) => {
            let mut it = a.iter().filter_map(|x| x.as_str().map(str::to_string));
            (it.next().unwrap_or_default(), it.collect())
        }
        _ => (String::new(), vec![]),
    };
    if let Some(a) = v.get("args").and_then(|a| a.as_array()) {
        args = a
            .iter()
            .filter_map(|x| x.as_str().map(str::to_string))
            .collect();
    }
    McpServer {
        name: name.to_string(),
        command,
        args,
        env: v
            .get("env")
            .or_else(|| v.get("environment"))
            .and_then(|e| e.as_object())
            .map(|o| {
                o.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default(),
        url: v
            .get("url")
            .and_then(|u| u.as_str())
            .unwrap_or_default()
            .to_string(),
        enabled: v.get("enabled").and_then(|e| e.as_bool()).unwrap_or(true),
    }
}

fn claude_value(s: &McpServer) -> serde_json::Value {
    let mut m = serde_json::Map::new();
    if !s.url.is_empty() {
        m.insert("type".into(), "http".into());
        m.insert("url".into(), s.url.clone().into());
    } else {
        m.insert("command".into(), s.command.clone().into());
        if !s.args.is_empty() {
            m.insert("args".into(), s.args.clone().into());
        }
    }
    if !s.env.is_empty() {
        m.insert(
            "env".into(),
            serde_json::Value::Object(
                s.env
                    .iter()
                    .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                    .collect(),
            ),
        );
    }
    serde_json::Value::Object(m)
}

fn opencode_value(s: &McpServer) -> serde_json::Value {
    let mut m = serde_json::Map::new();
    if !s.url.is_empty() {
        m.insert("type".into(), "remote".into());
        m.insert("url".into(), s.url.clone().into());
    } else {
        // OpenCode espera o comando como array [bin, ...args].
        let mut cmd = vec![s.command.clone()];
        cmd.extend(s.args.iter().cloned());
        m.insert("type".into(), "local".into());
        m.insert("command".into(), cmd.into());
    }
    if !s.env.is_empty() {
        m.insert(
            "environment".into(),
            serde_json::Value::Object(
                s.env
                    .iter()
                    .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                    .collect(),
            ),
        );
    }
    m.insert("enabled".into(), true.into());
    serde_json::Value::Object(m)
}

fn codex_value(s: &McpServer) -> toml::Value {
    let mut t = toml::Table::new();
    if !s.url.is_empty() {
        t.insert("url".into(), toml::Value::String(s.url.clone()));
    } else {
        t.insert("command".into(), toml::Value::String(s.command.clone()));
        if !s.args.is_empty() {
            t.insert(
                "args".into(),
                toml::Value::Array(s.args.iter().cloned().map(toml::Value::String).collect()),
            );
        }
    }
    if !s.env.is_empty() {
        let env: toml::Table = s
            .env
            .iter()
            .map(|(k, v)| (k.clone(), toml::Value::String(v.clone())))
            .collect();
        t.insert("env".into(), toml::Value::Table(env));
    }
    toml::Value::Table(t)
}

/// Mesmo contrato do `write_json_atomic`, para texto (TOML).
fn write_text_atomic(path: &Path, text: &str) -> Result<(), String> {
    use std::io::Write;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut tmp = path.as_os_str().to_os_string();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    {
        let mut f = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
        f.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
        f.sync_all().map_err(|e| e.to_string())?;
    }
    if path.exists() {
        let mut bak = path.as_os_str().to_os_string();
        bak.push(".bak");
        let _ = std::fs::copy(path, PathBuf::from(bak));
    }
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

fn read_skills_dir(dir: &Path, project_scoped: bool, shared: bool) -> Vec<Skill> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return vec![],
    };
    entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| read_skill(&e.path(), project_scoped, shared))
        .collect()
}

fn read_skill(dir: &Path, project_scoped: bool, shared: bool) -> Option<Skill> {
    let md = std::fs::read_to_string(dir.join("SKILL.md")).ok()?;
    let (name, description) = parse_frontmatter(&md);
    Some(Skill {
        name: name.unwrap_or_else(|| {
            dir.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("skill")
                .to_string()
        }),
        description: description.unwrap_or_default(),
        path: dir.to_string_lossy().to_string(),
        project_scoped,
        shared,
    })
}

/// Extrai `name`/`description` do frontmatter YAML do SKILL.md. Proposital não
/// puxar um parser de YAML: o frontmatter de skill é chave/valor raso, e uma dep
/// a mais no core não se paga por duas strings.
fn parse_frontmatter(md: &str) -> (Option<String>, Option<String>) {
    let mut lines = md.lines();
    if lines.next().map(str::trim) != Some("---") {
        return (None, None);
    }
    let (mut name, mut desc) = (None, None);
    for line in lines {
        let t = line.trim();
        if t == "---" {
            break;
        }
        if let Some(v) = t.strip_prefix("name:") {
            name = Some(unquote(v));
        } else if let Some(v) = t.strip_prefix("description:") {
            desc = Some(unquote(v));
        }
    }
    (name, desc)
}

fn unquote(s: &str) -> String {
    let t = s.trim();
    t.strip_prefix('"')
        .and_then(|x| x.strip_suffix('"'))
        .or_else(|| t.strip_prefix('\'').and_then(|x| x.strip_suffix('\'')))
        .unwrap_or(t)
        .to_string()
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let dest = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &dest)?;
        } else {
            std::fs::copy(entry.path(), dest)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn store_in(tmp: &TempDir) -> HarnessStore {
        let p = tmp.path();
        HarnessStore::new(HarnessPaths {
            claude_json: p.join(".claude.json"),
            codex_toml: p.join(".codex/config.toml"),
            opencode_json: p.join(".config/opencode/opencode.json"),
            skills_home: p.to_path_buf(),
            disabled_store: p.join("perene2/harness-disabled.json"),
        })
    }

    fn sample(name: &str) -> McpServer {
        McpServer {
            name: name.into(),
            command: "npx".into(),
            args: vec!["-y".into(), "server".into()],
            env: BTreeMap::from([("TOKEN".to_string(), "abc".to_string())]),
            url: String::new(),
            enabled: true,
        }
    }

    #[test]
    fn missing_files_list_nothing_instead_of_failing() {
        let tmp = TempDir::new().unwrap();
        let s = store_in(&tmp);
        for h in Harness::all() {
            assert!(s.list_mcp(h).unwrap().is_empty());
            assert!(!s.is_configured(h));
        }
    }

    #[test]
    fn claude_round_trip_preserves_unrelated_keys() {
        let tmp = TempDir::new().unwrap();
        let s = store_in(&tmp);
        // ~/.claude.json guarda MUITA coisa nossa que não é: tem que sobreviver.
        std::fs::write(
            &s.paths().claude_json,
            r##"{"hasCompletedOnboarding":true,"projects":{"/tmp/x":{"allowedTools":["Bash"]}}}"##,
        )
        .unwrap();

        s.upsert_mcp(Harness::Claude, &sample("ctx7")).unwrap();

        let raw = std::fs::read_to_string(&s.paths().claude_json).unwrap();
        let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(v["hasCompletedOnboarding"], serde_json::json!(true));
        assert_eq!(v["projects"]["/tmp/x"]["allowedTools"][0], "Bash");
        assert_eq!(v["mcpServers"]["ctx7"]["command"], "npx");
        assert_eq!(v["mcpServers"]["ctx7"]["env"]["TOKEN"], "abc");

        let listed = s.list_mcp(Harness::Claude).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].args, vec!["-y", "server"]);
    }

    #[test]
    fn codex_toml_round_trip_preserves_other_tables() {
        let tmp = TempDir::new().unwrap();
        let s = store_in(&tmp);
        std::fs::create_dir_all(s.paths().codex_toml.parent().unwrap()).unwrap();
        std::fs::write(
            &s.paths().codex_toml,
            "[tui]\nnotifications = true\nnotification_method = \"bel\"\n",
        )
        .unwrap();

        s.upsert_mcp(Harness::Codex, &sample("ctx7")).unwrap();

        let raw = std::fs::read_to_string(&s.paths().codex_toml).unwrap();
        let doc: toml::Value = toml::from_str(&raw).unwrap();
        assert_eq!(doc["tui"]["notifications"].as_bool(), Some(true));
        assert_eq!(doc["mcp_servers"]["ctx7"]["command"].as_str(), Some("npx"));

        let listed = s.list_mcp(Harness::Codex).unwrap();
        assert_eq!(listed[0].env.get("TOKEN").map(String::as_str), Some("abc"));
    }

    #[test]
    fn opencode_writes_command_as_array_and_reads_it_back() {
        let tmp = TempDir::new().unwrap();
        let s = store_in(&tmp);
        s.upsert_mcp(Harness::OpenCode, &sample("ctx7")).unwrap();

        let raw = std::fs::read_to_string(&s.paths().opencode_json).unwrap();
        let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(v["mcp"]["ctx7"]["type"], "local");
        assert_eq!(v["mcp"]["ctx7"]["command"][0], "npx");
        assert_eq!(v["mcp"]["ctx7"]["command"][2], "server");

        let listed = s.list_mcp(Harness::OpenCode).unwrap();
        assert_eq!(listed[0].command, "npx");
        assert_eq!(listed[0].args, vec!["-y", "server"]);
    }

    #[test]
    fn disabling_keeps_the_config_and_enabling_restores_it() {
        let tmp = TempDir::new().unwrap();
        let s = store_in(&tmp);
        s.upsert_mcp(Harness::Claude, &sample("ctx7")).unwrap();

        s.set_enabled(Harness::Claude, "ctx7", false).unwrap();
        // sumiu do arquivo da ferramenta...
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&s.paths().claude_json).unwrap())
                .unwrap();
        assert!(v["mcpServers"].get("ctx7").is_none());
        // ...mas continua listado, marcado como desligado
        let listed = s.list_mcp(Harness::Claude).unwrap();
        assert_eq!(listed.len(), 1);
        assert!(!listed[0].enabled);
        assert_eq!(listed[0].env.get("TOKEN").map(String::as_str), Some("abc"));

        s.set_enabled(Harness::Claude, "ctx7", true).unwrap();
        let listed = s.list_mcp(Harness::Claude).unwrap();
        assert!(listed[0].enabled);
        assert_eq!(
            listed[0].args,
            vec!["-y", "server"],
            "args não podem se perder"
        );
    }

    #[test]
    fn remove_clears_both_the_tool_file_and_our_disabled_list() {
        let tmp = TempDir::new().unwrap();
        let s = store_in(&tmp);
        s.upsert_mcp(Harness::Claude, &sample("ctx7")).unwrap();
        s.set_enabled(Harness::Claude, "ctx7", false).unwrap();
        s.remove_mcp(Harness::Claude, "ctx7").unwrap();
        assert!(s.list_mcp(Harness::Claude).unwrap().is_empty());
    }

    /// Um ciclo completo (add → desliga → liga → remove) tem que devolver o
    /// arquivo do usuário EXATAMENTE como estava — mesmo texto, byte a byte.
    ///
    /// Pegou três coisas que o fixture curto não pegava, todas contra o
    /// ~/.claude.json real de 42 KB: as chaves saíam alfabetizadas, os floats
    /// longos voltavam com 1 ULP de diferença, e sobrava um `"mcpServers": {}`
    /// num arquivo que não tinha a chave.
    #[test]
    fn full_cycle_leaves_the_user_file_byte_identical() {
        let tmp = TempDir::new().unwrap();
        let s = store_in(&tmp);

        // Ordem NÃO alfabética + float com mais dígitos do que f64 representa,
        // que é o formato real do arquivo do Claude Code.
        let original = concat!(
            "{\n",
            "  \"numStartups\": 42,\n",
            "  \"installMethod\": \"native\",\n",
            "  \"autoCompactWindowsCache\": {},\n",
            "  \"projects\": {\n",
            "    \"/home/user\": {\n",
            "      \"allowedTools\": [],\n",
            "      \"lastSessionMetrics\": {\n",
            "        \"frame_duration_ms_p50\": 0.40265999999974156,\n",
            "        \"hook_duration_ms_avg\": 0.10416666666666667\n",
            "      }\n",
            "    }\n",
            "  }\n",
            "}"
        );
        std::fs::write(&s.paths().claude_json, original).unwrap();

        s.upsert_mcp(Harness::Claude, &sample("ctx7")).unwrap();
        s.set_enabled(Harness::Claude, "ctx7", false).unwrap();
        s.set_enabled(Harness::Claude, "ctx7", true).unwrap();
        s.remove_mcp(Harness::Claude, "ctx7").unwrap();

        let depois = std::fs::read_to_string(&s.paths().claude_json).unwrap();
        let v: serde_json::Value = serde_json::from_str(&depois).unwrap();

        assert!(
            v.get("mcpServers").is_none(),
            "não pode sobrar mcpServers vazio: {depois}"
        );
        // ordem preservada
        let keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(
            keys,
            vec![
                "numStartups",
                "installMethod",
                "autoCompactWindowsCache",
                "projects"
            ],
            "a ordem das chaves do usuário mudou"
        );
        // O texto tem que ser idêntico, não só equivalente: é arquivo alheio.
        assert_eq!(depois.trim(), original.trim(), "o arquivo do usuário mudou");
        // floats intactos, no texto
        assert!(
            depois.contains("0.40265999999974156"),
            "float perdeu precisão: {depois}"
        );
        assert!(
            depois.contains("0.10416666666666667"),
            "float perdeu precisão"
        );
    }

    #[test]
    fn codex_removal_does_not_leave_an_empty_table() {
        let tmp = TempDir::new().unwrap();
        let s = store_in(&tmp);
        std::fs::create_dir_all(s.paths().codex_toml.parent().unwrap()).unwrap();
        std::fs::write(&s.paths().codex_toml, "[tui]\nnotifications = true\n").unwrap();

        s.upsert_mcp(Harness::Codex, &sample("ctx7")).unwrap();
        s.remove_mcp(Harness::Codex, "ctx7").unwrap();

        let depois = std::fs::read_to_string(&s.paths().codex_toml).unwrap();
        assert!(
            !depois.contains("mcp_servers"),
            "sobrou seção vazia: {depois}"
        );
        assert!(
            depois.contains("notifications"),
            "comeu a config do usuário"
        );
    }

    #[test]
    fn upsert_validates_input() {
        let tmp = TempDir::new().unwrap();
        let s = store_in(&tmp);
        let mut bad = sample("x");
        bad.name = " ".into();
        assert!(s.upsert_mcp(Harness::Claude, &bad).is_err());

        let mut no_cmd = sample("y");
        no_cmd.command = String::new();
        assert!(s.upsert_mcp(Harness::Claude, &no_cmd).is_err());
    }

    #[test]
    fn remote_servers_use_url() {
        let tmp = TempDir::new().unwrap();
        let s = store_in(&tmp);
        let remote = McpServer {
            name: "api".into(),
            command: String::new(),
            args: vec![],
            env: BTreeMap::new(),
            url: "https://example.com/mcp".into(),
            enabled: true,
        };
        s.upsert_mcp(Harness::Claude, &remote).unwrap();
        let listed = s.list_mcp(Harness::Claude).unwrap();
        assert_eq!(listed[0].url, "https://example.com/mcp");
    }

    #[test]
    fn skills_are_read_from_frontmatter() {
        let tmp = TempDir::new().unwrap();
        let s = store_in(&tmp);
        let dir = s.paths().skills_home.join(".claude/skills/deploy");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("SKILL.md"),
            "---\nname: deploy\ndescription: \"Sobe a app\"\n---\n\nConteúdo",
        )
        .unwrap();

        let skills = s.list_skills(Harness::Claude, None);
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "deploy");
        assert_eq!(skills[0].description, "Sobe a app");
        assert!(!skills[0].project_scoped);
    }

    #[test]
    fn install_skill_copies_and_refuses_without_skill_md() {
        let tmp = TempDir::new().unwrap();
        let s = store_in(&tmp);

        let src = tmp.path().join("origem/minha-skill");
        std::fs::create_dir_all(src.join("sub")).unwrap();
        std::fs::write(src.join("SKILL.md"), "---\nname: minha-skill\n---\n").unwrap();
        std::fs::write(src.join("sub/extra.txt"), "x").unwrap();

        let skill = s.install_skill(Harness::Claude, &src, None).unwrap();
        assert_eq!(skill.name, "minha-skill");
        assert!(s
            .paths()
            .skills_home
            .join(".claude/skills/minha-skill/sub/extra.txt")
            .exists());

        // duas vezes não
        assert!(s.install_skill(Harness::Claude, &src, None).is_err());

        let vazio = tmp.path().join("vazio");
        std::fs::create_dir_all(&vazio).unwrap();
        assert!(s.install_skill(Harness::Claude, &vazio, None).is_err());
    }

    #[test]
    fn remove_skill_refuses_paths_outside_the_skills_dirs() {
        let tmp = TempDir::new().unwrap();
        let s = store_in(&tmp);
        let fora = tmp.path().join("projeto-importante");
        std::fs::create_dir_all(&fora).unwrap();

        assert!(s.remove_skill(Harness::Claude, &fora, None).is_err());
        assert!(fora.exists(), "não pode ter apagado nada");
        // o próprio diretório de skills também não
        let raiz = s.paths().skills_home.join(".claude/skills");
        assert!(s.remove_skill(Harness::Claude, &raiz, None).is_err());
    }

    #[test]
    fn project_skills_are_listed_separately() {
        let tmp = TempDir::new().unwrap();
        let s = store_in(&tmp);
        let proj = tmp.path().join("repo");
        let dir = proj.join(".claude/skills/local");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("SKILL.md"), "---\nname: local\n---\n").unwrap();

        assert!(s.list_skills(Harness::Claude, None).is_empty());
        let with_project = s.list_skills(Harness::Claude, Some(&proj));
        assert_eq!(with_project.len(), 1);
        assert!(with_project[0].project_scoped);
    }

    /// Cria uma skill mínima em `<raiz>/<sub>/skills/<nome>`.
    fn skill_em(raiz: &Path, sub: &str, nome: &str) {
        let dir = raiz.join(sub).join("skills").join(nome);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("SKILL.md"), format!("---\nname: {nome}\n---\n")).unwrap();
    }

    #[test]
    fn codex_and_opencode_also_read_the_shared_agents_dir() {
        // A skill em `.agents/skills` serve as duas ferramentas sem cópia — foi
        // por não ler daqui que a lista aparecia vazia para elas.
        let tmp = TempDir::new().unwrap();
        let s = store_in(&tmp);
        let home = s.paths().skills_home.clone();
        skill_em(&home, ".agents", "compartilhada");

        for h in [Harness::Codex, Harness::OpenCode] {
            let nomes: Vec<_> = s.list_skills(h, None).into_iter().map(|k| k.name).collect();
            assert_eq!(nomes, vec!["compartilhada"], "{h:?} devia enxergar");
        }
        // O claude não lê daqui: só `.claude/skills`.
        assert!(s.list_skills(Harness::Claude, None).is_empty());
    }

    #[test]
    fn each_tool_also_reads_its_own_dir() {
        let tmp = TempDir::new().unwrap();
        let s = store_in(&tmp);
        let home = s.paths().skills_home.clone();
        skill_em(&home, ".claude", "so-claude");
        skill_em(&home, ".codex", "so-codex");
        skill_em(&home, ".opencode", "so-opencode");

        let nomes =
            |h| -> Vec<String> { s.list_skills(h, None).into_iter().map(|k| k.name).collect() };
        assert_eq!(nomes(Harness::Claude), vec!["so-claude"]);
        assert_eq!(nomes(Harness::Codex), vec!["so-codex"]);
        assert_eq!(nomes(Harness::OpenCode), vec!["so-opencode"]);
    }

    #[test]
    fn shared_skills_are_flagged_so_the_ui_can_warn() {
        // Mexer numa skill de `.agents/skills` afeta mais de uma ferramenta; a
        // UI precisa saber disso para avisar antes de remover.
        let tmp = TempDir::new().unwrap();
        let s = store_in(&tmp);
        let home = s.paths().skills_home.clone();
        skill_em(&home, ".agents", "compartilhada");
        skill_em(&home, ".codex", "propria");

        let skills = s.list_skills(Harness::Codex, None);
        let compartilhada = skills.iter().find(|k| k.name == "compartilhada").unwrap();
        let propria = skills.iter().find(|k| k.name == "propria").unwrap();
        assert!(compartilhada.shared);
        assert!(!propria.shared);
    }

    #[test]
    fn installing_for_codex_lands_in_the_shared_dir() {
        // Instalar uma vez e as duas ferramentas enxergarem é o comportamento
        // útil; por isso o destino padrão delas é o compartilhado.
        let tmp = TempDir::new().unwrap();
        let s = store_in(&tmp);
        let src = tmp.path().join("origem/nova");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("SKILL.md"), "---\nname: nova\n---\n").unwrap();

        let skill = s.install_skill(Harness::Codex, &src, None).unwrap();
        assert!(skill.shared);
        assert!(s.paths().skills_home.join(".agents/skills/nova").exists());
        // E o opencode já enxerga, sem instalar de novo.
        assert!(s
            .list_skills(Harness::OpenCode, None)
            .iter()
            .any(|k| k.name == "nova"));
    }

    #[test]
    fn remove_accepts_the_shared_dir_but_still_refuses_outside() {
        let tmp = TempDir::new().unwrap();
        let s = store_in(&tmp);
        let home = s.paths().skills_home.clone();
        skill_em(&home, ".agents", "descartavel");
        let alvo = home.join(".agents/skills/descartavel");

        // Fora continua recusado, mesmo para uma ferramenta que lê o compartilhado.
        let fora = tmp.path().join("importante");
        std::fs::create_dir_all(&fora).unwrap();
        assert!(s.remove_skill(Harness::Codex, &fora, None).is_err());
        assert!(fora.exists());

        s.remove_skill(Harness::Codex, &alvo, None).unwrap();
        assert!(!alvo.exists());
    }
}
