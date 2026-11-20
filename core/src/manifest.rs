//! Manifesto do Ludusavi (dados do PCGamingWiki): onde cada jogo de PC guarda saves.

use anyhow::{Context, Result};
use serde::{de::IgnoredAny, Deserialize};
use std::{
    collections::BTreeMap,
    fs,
    io::{BufReader, Read},
    path::Path,
};

pub const URL: &str =
    "https://raw.githubusercontent.com/mtkennerly/ludusavi-manifest/master/data/manifest.yaml";

pub type Manifest = BTreeMap<String, Game>;

#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Game {
    pub files: BTreeMap<String, Option<Rule>>,
    pub registry: BTreeMap<String, Option<Rule>>,
    pub install_dir: BTreeMap<String, IgnoredAny>,
    pub launch: BTreeMap<String, IgnoredAny>,
    pub steam: Option<Id>,
    pub gog: Option<Id>,
    // ---- campos próprios (extras.yaml) ----
    /// Executáveis relativos a `<base>`, procurados nas pastas raiz.
    pub exe: Vec<String>,
    /// Arquivo de config -> chave -> padrões dentro da pasta apontada.
    pub config_dirs: BTreeMap<String, BTreeMap<String, Vec<String>>>,
    pub sidecar: Option<Sidecar>,
}

/// Saves com o mesmo nome da ROM, na mesma pasta.
#[derive(Default, Deserialize)]
#[serde(default)]
pub struct Sidecar {
    pub roms: Vec<String>,
    pub saves: Vec<String>,
}

pub const EXTRAS: &str = include_str!("../data/extras.yaml");

/// Aplica `extras.yaml` (emuladores e ports) por cima do manifesto.
pub fn with_extras(mut m: Manifest) -> Result<Manifest> {
    for (name, e) in parse(EXTRAS).context("extras.yaml")? {
        let g = m.entry(name).or_default();
        if !e.files.is_empty() {
            g.files = e.files;
        }
        g.exe = e.exe;
        g.config_dirs = e.config_dirs;
        g.sidecar = e.sidecar;
    }
    Ok(m)
}

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct Rule {
    pub when: Vec<When>,
}

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct When {
    pub os: Option<String>,
    pub store: Option<String>,
}

#[derive(Deserialize)]
pub struct Id {
    pub id: Option<u64>,
}

pub fn parse(yaml: &str) -> Result<Manifest> {
    serde_yaml::from_str(yaml).context("manifesto inválido")
}

pub fn load(path: &Path) -> Result<Manifest> {
    let f = fs::File::open(path).with_context(|| {
        format!("manifesto não encontrado em {} — rode `ugs update-manifest`", path.display())
    })?;
    with_extras(serde_yaml::from_reader(BufReader::new(f)).context("manifesto inválido")?)
}

/// Baixa o manifesto se mudou (ETag). Retorna `true` se atualizou.
pub fn update(path: &Path) -> Result<bool> {
    let etag_path = path.with_extension("etag");
    let mut req = ureq::get(URL);
    if path.exists() {
        if let Ok(e) = fs::read_to_string(&etag_path) {
            req = req.set("If-None-Match", e.trim());
        }
    }
    let resp = req.call().context("falha ao baixar manifesto")?;
    if resp.status() == 304 {
        return Ok(false);
    }
    let etag = resp.header("ETag").map(str::to_owned);
    let mut body = String::new();
    resp.into_reader().take(256 << 20).read_to_string(&mut body)?;
    parse(&body)?; // não substitui o cache por um arquivo quebrado

    fs::create_dir_all(path.parent().context("caminho do manifesto sem pasta")?)?;
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, &body)?;
    fs::rename(&tmp, path)?;
    if let Some(e) = etag {
        fs::write(etag_path, e)?;
    }
    Ok(true)
}
