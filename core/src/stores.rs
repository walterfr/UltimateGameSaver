//! Jogos instalados via Steam, Epic, GOG e pastas raiz do usuário (emuladores, ports, ROMs).

use crate::{manifest::Manifest, scan::norm};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Store {
    Steam,
    Epic,
    Gog,
    /// Achado pelo executável numa pasta raiz.
    Other,
}

impl Store {
    pub fn key(self) -> &'static str {
        match self {
            Store::Steam => "steam",
            Store::Epic => "epic",
            Store::Gog => "gog",
            Store::Other => "other",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Installed {
    pub store: Store,
    pub dir: PathBuf,
    pub steam_id: Option<u64>,
    pub gog_id: Option<u64>,
    /// Nome no manifesto, quando achado pelo executável.
    pub game: Option<String>,
}

impl Installed {
    pub fn folder(&self) -> String {
        self.dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
    }
}

#[derive(Debug, Default)]
pub struct Stores {
    pub steam_root: Option<PathBuf>,
    pub games: Vec<Installed>,
    /// Jogo do manifesto -> saves achados ao lado de ROMs.
    pub sidecars: HashMap<String, Vec<PathBuf>>,
}

/// Profundidade máxima ao varrer pastas raiz.
const ROOT_DEPTH: usize = 6;

pub fn detect(roots: &[PathBuf], manifest: &Manifest) -> Stores {
    let mut s = Stores::default();
    if let Some(root) = steam_root() {
        steam(&root, &mut s);
        s.steam_root = Some(root);
    }
    epic(&mut s);
    gog(&mut s);
    if !roots.is_empty() {
        let idx = RootIndex::new(manifest);
        let mut seen = HashSet::new();
        for r in roots {
            walk_root(r, ROOT_DEPTH, &idx, &mut s, &mut seen);
        }
    }
    s
}

struct RootIndex<'a> {
    /// nome do arquivo (minúsculo) -> (jogo, caminho relativo a <base>, minúsculo)
    exes: HashMap<String, Vec<(&'a str, String)>>,
    /// (jogo, extensões de ROM, extensões de save)
    sidecars: Vec<(&'a str, HashSet<String>, HashSet<String>)>,
}

impl<'a> RootIndex<'a> {
    fn new(m: &'a Manifest) -> Self {
        let mut exes: HashMap<String, Vec<(&str, String)>> = HashMap::new();
        let file_name = |rel: &str| rel.rsplit('/').next().unwrap_or(rel).to_lowercase();
        // `launch` do manifesto: só nomes de executável que pertencem a um único jogo
        let mut launch: HashMap<String, Vec<(&str, String)>> = HashMap::new();
        for (name, g) in m {
            for k in g.launch.keys() {
                if let Some(rel) = k.strip_prefix("<base>/").filter(|r| !r.contains('<')) {
                    launch.entry(file_name(rel)).or_default().push((name, rel.to_lowercase()));
                }
            }
        }
        for (exe, v) in launch {
            if v.iter().all(|(n, _)| *n == v[0].0) {
                exes.insert(exe, v);
            }
        }
        let mut sidecars = Vec::new();
        for (name, g) in m {
            for rel in &g.exe {
                exes.entry(file_name(rel)).or_default().push((name, rel.to_lowercase()));
            }
            if let Some(sc) = &g.sidecar {
                let set = |v: &[String]| v.iter().map(|x| x.to_lowercase()).collect();
                sidecars.push((name.as_str(), set(&sc.roms), set(&sc.saves)));
            }
        }
        RootIndex { exes, sidecars }
    }
}

fn walk_root(dir: &Path, depth: usize, idx: &RootIndex, s: &mut Stores, seen: &mut HashSet<(String, String)>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    let (mut files, mut dirs) = (Vec::new(), Vec::new());
    for e in rd.flatten() {
        match e.file_type() {
            Ok(t) if t.is_dir() => dirs.push(e.path()),
            Ok(t) if t.is_file() => files.push(e.path()),
            _ => {}
        }
    }
    let lower_parts = |p: &Path| {
        let stem = p.file_stem().map(|x| x.to_string_lossy().to_lowercase()).unwrap_or_default();
        let ext = p.extension().map(|x| x.to_string_lossy().to_lowercase()).unwrap_or_default();
        (stem, ext)
    };

    for f in &files {
        let name = f.file_name().unwrap().to_string_lossy().to_lowercase();
        let Some(hits) = idx.exes.get(&name) else { continue };
        let full = norm(f).to_lowercase();
        for (game, rel) in hits {
            if full.ends_with(&format!("/{rel}")) {
                let Some(base) = f.ancestors().nth(rel.split('/').count()) else { continue };
                if seen.insert((game.to_string(), norm(base).to_lowercase())) {
                    s.games.push(Installed {
                        store: Store::Other,
                        dir: base.to_path_buf(),
                        steam_id: None,
                        gog_id: None,
                        game: Some(game.to_string()),
                    });
                }
            }
        }
    }

    for (game, roms, saves) in &idx.sidecars {
        let rom_stems: HashSet<String> =
            files.iter().map(|f| lower_parts(f)).filter(|(_, e)| roms.contains(e)).map(|(st, _)| st).collect();
        if rom_stems.is_empty() {
            continue;
        }
        for f in &files {
            let (stem, ext) = lower_parts(f);
            if saves.contains(&ext) && rom_stems.contains(&stem) {
                s.sidecars.entry(game.to_string()).or_default().push(f.clone());
            }
        }
    }

    if depth > 0 {
        for d in dirs {
            walk_root(&d, depth - 1, idx, s, seen);
        }
    }
}

fn steam_root() -> Option<PathBuf> {
    #[cfg(windows)]
    let p = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
        .open_subkey("Software\\Valve\\Steam")
        .and_then(|k| k.get_value::<String, _>("SteamPath"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("C:/Program Files (x86)/Steam"));
    #[cfg(not(windows))]
    let p = dirs::home_dir()?.join(".local/share/Steam");
    p.is_dir().then_some(p)
}

fn steam(root: &Path, s: &mut Stores) {
    let mut libs = vec![root.to_path_buf()];
    if let Ok(t) = fs::read_to_string(root.join("steamapps/libraryfolders.vdf")) {
        libs.extend(vdf_values(&t, "path").map(PathBuf::from));
    }
    let mut seen = HashSet::new();
    for lib in libs {
        let Ok(rd) = fs::read_dir(lib.join("steamapps")) else { continue };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if !(name.starts_with("appmanifest_") && name.ends_with(".acf")) {
                continue;
            }
            let Ok(t) = fs::read_to_string(e.path()) else { continue };
            let id = vdf_values(&t, "appid").next().and_then(|v| v.parse().ok());
            let Some(dir) = vdf_values(&t, "installdir").next() else { continue };
            let dir = lib.join("steamapps/common").join(dir);
            // a biblioteca principal aparece duas vezes (registro + vdf)
            if dir.is_dir() && seen.insert(id) {
                s.games.push(Installed { store: Store::Steam, dir, steam_id: id, gog_id: None, game: None });
            }
        }
    }
}

/// Valores de `"chave"  "valor"` num arquivo VDF/ACF da Steam.
fn vdf_values<'a>(text: &'a str, key: &'a str) -> impl Iterator<Item = String> + 'a {
    text.lines().filter_map(move |l| {
        let mut q = l.split('"');
        q.next()?;
        let k = q.next()?;
        q.next()?;
        let v = q.next()?;
        k.eq_ignore_ascii_case(key).then(|| v.replace("\\\\", "\\"))
    })
}

fn epic(s: &mut Stores) {
    let Some(pd) = std::env::var_os("ProgramData") else { return };
    let dir = PathBuf::from(pd).join("Epic/EpicGamesLauncher/Data/Manifests");
    let Ok(rd) = fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        if e.path().extension().is_some_and(|x| x == "item") {
            let Ok(t) = fs::read_to_string(e.path()) else { continue };
            let Ok(v) = serde_json::from_str::<serde_json::Value>(&t) else { continue };
            if let Some(loc) = v["InstallLocation"].as_str() {
                s.games.push(Installed { store: Store::Epic, dir: loc.into(), steam_id: None, gog_id: None, game: None });
            }
        }
    }
}

#[cfg(windows)]
fn gog(s: &mut Stores) {
    use winreg::{enums::HKEY_LOCAL_MACHINE, RegKey};
    let Ok(games) = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey("SOFTWARE\\WOW6432Node\\GOG.com\\Games")
    else {
        return;
    };
    for name in games.enum_keys().flatten() {
        let Ok(k) = games.open_subkey(&name) else { continue };
        let Ok(path) = k.get_value::<String, _>("path") else { continue };
        let id = k.get_value::<String, _>("gameID").ok().and_then(|v| v.parse().ok());
        s.games.push(Installed { store: Store::Gog, dir: path.into(), steam_id: None, gog_id: id, game: None });
    }
}

#[cfg(not(windows))]
fn gog(_: &mut Stores) {}

#[cfg(test)]
mod tests {
    #[test]
    fn vdf() {
        let t = "\"0\"\n{\n\t\t\"path\"\t\t\"D:\\\\SteamLibrary\"\n\t\t\"label\"\t\t\"\"\n}";
        assert_eq!(super::vdf_values(t, "path").collect::<Vec<_>>(), ["D:\\SteamLibrary"]);
    }
}
