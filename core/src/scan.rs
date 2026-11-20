//! Expande placeholders do manifesto e acha os arquivos de save que existem no disco.

use crate::{
    manifest::{Game, Manifest, Rule},
    stores::{Store, Stores},
};
use glob::{MatchOptions, Pattern};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

const OS: &str = if cfg!(windows) {
    "windows"
} else if cfg!(target_os = "macos") {
    "mac"
} else {
    "linux"
};

/// Pastas do usuário atual, usadas para expandir e generalizar caminhos.
pub struct Env {
    /// (placeholder, caminho com `/`, sem barra final)
    pub vars: Vec<(&'static str, String)>,
    pub user: String,
}

impl Env {
    pub fn detect() -> Self {
        let home = dirs::home_dir();
        let mut vars = Vec::new();
        let mut add = |k, p: Option<PathBuf>| {
            if let Some(p) = p {
                vars.push((k, norm(&p)));
            }
        };
        #[cfg(windows)]
        {
            let env = |k| std::env::var_os(k).map(PathBuf::from);
            add("<winLocalAppDataLow>", home.as_ref().map(|h| h.join("AppData/LocalLow")));
            add("<winLocalAppData>", dirs::data_local_dir());
            add("<winAppData>", dirs::data_dir());
            add("<winDocuments>", dirs::document_dir());
            add("<winPublic>", env("PUBLIC"));
            add("<winProgramData>", env("ProgramData"));
            add("<winDir>", env("WINDIR"));
        }
        #[cfg(not(windows))]
        {
            add("<xdgData>", dirs::data_dir());
            add("<xdgConfig>", dirs::config_dir());
        }
        add("<home>", home);
        let user = std::env::var("USERNAME").or_else(|_| std::env::var("USER")).unwrap_or_default();
        Env { vars, user }
    }

    pub fn get(&self, placeholder: &str) -> Option<&str> {
        self.vars.iter().find(|(k, _)| *k == placeholder).map(|(_, v)| v.as_str())
    }
}

#[derive(Debug, Default)]
pub struct Found {
    pub name: String,
    /// Pasta de instalação (`<base>`), se o jogo foi achado em alguma loja.
    pub base: Option<PathBuf>,
    pub files: Vec<PathBuf>,
    /// Chaves `HKEY_CURRENT_USER/...` que existem.
    pub registry: Vec<String>,
}

pub fn scan(manifest: &Manifest, env: &Env, stores: &Stores, filter: Option<&str>) -> Vec<Found> {
    let filter = filter.map(str::to_lowercase);
    manifest
        .iter()
        .filter(|(n, _)| filter.as_ref().map_or(true, |f| n.to_lowercase().contains(f)))
        .map(|(n, g)| resolve(n, g, env, stores))
        .filter(|f| !f.files.is_empty() || !f.registry.is_empty())
        .collect()
}

/// Acha um jogo pelo nome exato (sem diferenciar maiúsculas).
pub fn find<'a>(manifest: &'a Manifest, name: &str) -> Option<(&'a String, &'a Game)> {
    let name = name.to_lowercase();
    manifest.iter().find(|(k, _)| k.to_lowercase() == name)
}

pub fn resolve(name: &str, game: &Game, env: &Env, stores: &Stores) -> Found {
    let mut files = BTreeSet::new();
    let mut base = None;
    for ctx in contexts(name, game, stores, env) {
        if base.is_none() {
            base = ctx.base.as_ref().map(PathBuf::from);
        }
        for (pattern, rule) in &game.files {
            if applies(rule.as_ref(), ctx.store, ctx.os()) {
                if let Some(p) = expand(pattern, env, &ctx) {
                    collect(&p, &mut files, ctx.os() == "windows");
                }
            }
        }
        for (cfg, keys) in &game.config_dirs {
            let Some(cfg) = expand(cfg, env, &ctx) else { continue };
            for cfg in glob::glob(&cfg).into_iter().flatten().flatten() {
                let Ok(text) = fs::read_to_string(&cfg) else { continue };
                for (key, patterns) in keys {
                    if let Some(dir) = cfg_dir(&text, key, &cfg) {
                        for p in patterns {
                            collect(&format!("{}/{p}", Pattern::escape(&norm(&dir))), &mut files, cfg!(windows));
                        }
                    }
                }
            }
        }
    }
    files.extend(stores.sidecars.get(name).into_iter().flatten().cloned());
    let registry = game
        .registry
        .iter()
        .filter(|(k, r)| applies(r.as_ref(), None, OS) && !k.contains('*') && reg_exists(k))
        .map(|(k, _)| k.clone())
        .collect();
    Found { name: name.to_string(), base, files: files.into_iter().collect(), registry }
}

#[derive(Default)]
struct Ctx {
    store: Option<Store>,
    root: Option<String>,
    base: Option<String>,
    game: Option<String>,
    game_id: Option<String>,
    /// Placeholders próprios deste contexto (prefixo Wine/Proton); têm prioridade sobre `Env`.
    vars: Vec<(&'static str, String)>,
}

impl Ctx {
    /// Dentro de um prefixo Wine as regras de Windows valem, mesmo rodando no Linux.
    fn os(&self) -> &'static str {
        if self.vars.is_empty() {
            OS
        } else {
            "windows"
        }
    }
}

/// Pastas Windows dentro de um prefixo Wine/Proton (`<prefixo>/drive_c/users/<usuário>`).
fn wine_vars(prefix: &Path, user: &str) -> Vec<(&'static str, String)> {
    let c = prefix.join("drive_c");
    let users = c.join("users");
    let home = [user, "steamuser"].iter().map(|u| users.join(u)).find(|p| p.is_dir()).unwrap_or_else(|| users.join("steamuser"));
    let h = norm(&home);
    vec![
        ("<home>", h.clone()),
        ("<winAppData>", format!("{h}/AppData/Roaming")),
        ("<winLocalAppData>", format!("{h}/AppData/Local")),
        ("<winLocalAppDataLow>", format!("{h}/AppData/LocalLow")),
        ("<winDocuments>", format!("{h}/Documents")),
        ("<winPublic>", norm(&users.join("Public"))),
        ("<winProgramData>", norm(&c.join("ProgramData"))),
        ("<winDir>", norm(&c.join("windows"))),
        ("<osUserName>", home.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()),
    ]
}

/// Uma combinação de loja/instalação por onde procurar; sempre inclui o contexto genérico.
fn contexts(name: &str, game: &Game, stores: &Stores, env: &Env) -> Vec<Ctx> {
    let steam_id = game.steam.as_ref().and_then(|i| i.id);
    let gog_id = game.gog.as_ref().and_then(|i| i.id);
    let wine = |p: Option<&Path>| p.map(|p| wine_vars(p, &env.user)).unwrap_or_default();
    let mut v: Vec<Ctx> = stores
        .games
        .iter()
        .filter(|g| match &g.game {
            Some(found_for) => found_for == name,
            None => {
                (g.steam_id.is_some() && g.steam_id == steam_id)
                    || (g.gog_id.is_some() && g.gog_id == gog_id)
                    || game.install_dir.keys().any(|k| k.eq_ignore_ascii_case(&g.folder()))
            }
        })
        .map(|g| Ctx {
            store: Some(g.store),
            root: match g.store {
                Store::Steam => stores.steam_root.as_deref().map(norm),
                _ => g.dir.parent().map(norm),
            },
            base: Some(norm(&g.dir)),
            game: Some(g.folder()),
            game_id: g.steam_id.or(g.gog_id).map(|i| i.to_string()),
            vars: wine(g.prefix.as_deref()),
        })
        .collect();
    // Steam Cloud (`<root>/userdata/...`) e prefixo Proton existem mesmo com o jogo desinstalado.
    if let (Some(root), Some(id)) = (&stores.steam_root, steam_id) {
        if !v.iter().any(|c| c.store == Some(Store::Steam)) {
            v.push(Ctx {
                store: Some(Store::Steam),
                root: Some(norm(root)),
                game_id: Some(id.to_string()),
                vars: wine(crate::stores::proton_prefix(root, id).as_deref()),
                ..Ctx::default()
            });
        }
    }
    v.push(Ctx::default());
    v
}

fn applies(rule: Option<&Rule>, store: Option<Store>, os: &str) -> bool {
    let Some(r) = rule else { return true };
    r.when.is_empty()
        || r.when.iter().any(|w| {
            w.os.as_deref().map_or(true, |o| o == os)
                && w.store.as_deref().map_or(true, |st| store.map_or(true, |s| s.key() == st))
        })
}

/// Troca placeholders por valores (escapados para glob). `None` se faltar algum.
fn expand(pattern: &str, env: &Env, ctx: &Ctx) -> Option<String> {
    let mut out = String::new();
    let mut rest = pattern;
    while let Some(i) = rest.find('<') {
        out.push_str(&rest[..i]);
        let j = i + rest[i..].find('>')?;
        let value = match &rest[i..=j] {
            "<base>" => Pattern::escape(ctx.base.as_deref()?),
            "<root>" => Pattern::escape(ctx.root.as_deref()?),
            "<game>" => Pattern::escape(ctx.game.as_deref()?),
            "<storeGameId>" => ctx.game_id.clone().unwrap_or_else(|| "*".into()),
            "<storeUserId>" => "*".into(),
            ph => match ctx.vars.iter().find(|(k, _)| *k == ph) {
                Some((_, v)) => Pattern::escape(v),
                None if ph == "<osUserName>" => Pattern::escape(&env.user),
                None => Pattern::escape(env.get(ph)?),
            },
        };
        out.push_str(&value);
        rest = &rest[j + 1..];
    }
    out.push_str(rest);
    Some(out)
}

/// Pasta apontada por `key` num arquivo de config (`key = valor`, `key: valor` ou `<key>valor</key>`).
fn cfg_dir(text: &str, key: &str, cfg: &Path) -> Option<PathBuf> {
    let value = text.lines().find_map(|l| {
        let l = l.trim();
        if let Some(rest) = l.strip_prefix(&format!("<{key}>")) {
            return rest.split("</").next().map(str::to_string);
        }
        let rest = l.strip_prefix(key)?.trim_start();
        let v = rest.strip_prefix('=').or_else(|| rest.strip_prefix(':'))?;
        Some(v.trim().trim_matches(['"', '\'']).to_string())
    })?;
    let value = value.trim();
    if value.is_empty() || value.eq_ignore_ascii_case("default") {
        return None;
    }
    let here = cfg.parent()?;
    let dir = if let Some(rel) = value.strip_prefix(':') {
        here.join(rel.trim_start_matches(['/', '\\']))
    } else if let Some(rel) = value.strip_prefix("~/") {
        dirs::home_dir()?.join(rel)
    } else {
        here.join(value) // absoluto substitui `here`
    };
    dir.is_dir().then_some(dir)
}

/// `ignore_case`: no Windows e em prefixos Wine (regras de Windows) maiúsculas não importam.
fn collect(pattern: &str, out: &mut BTreeSet<PathBuf>, ignore_case: bool) {
    let opts = MatchOptions {
        case_sensitive: !ignore_case,
        require_literal_separator: true,
        require_literal_leading_dot: false,
    };
    let Ok(paths) = glob::glob_with(pattern, opts) else { return };
    for p in paths.flatten() {
        if p.is_file() {
            out.insert(p);
        } else if p.is_dir() {
            walk(&p, out);
        }
    }
}

fn walk(dir: &Path, out: &mut BTreeSet<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        match e.file_type() {
            Ok(t) if t.is_dir() => walk(&e.path(), out),
            Ok(t) if t.is_file() => {
                out.insert(e.path());
            }
            _ => {}
        }
    }
}

#[cfg(windows)]
fn reg_exists(key: &str) -> bool {
    use winreg::{enums::HKEY_CURRENT_USER, RegKey};
    // ponytail: só HKCU; HKLM exige admin para restaurar.
    match key.split_once('/') {
        Some(("HKEY_CURRENT_USER", sub)) => {
            RegKey::predef(HKEY_CURRENT_USER).open_subkey(sub.replace('/', "\\")).is_ok()
        }
        _ => false,
    }
}

#[cfg(not(windows))]
fn reg_exists(_: &str) -> bool {
    false
}

pub fn norm(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/").trim_end_matches('/').to_string()
}

/// `p` está dentro da pasta `dir`? (sem diferenciar maiúsculas no Windows)
pub fn is_under(p: &str, dir: &str) -> bool {
    p.len() > dir.len()
        && p.as_bytes()[dir.len()] == b'/'
        && p.is_char_boundary(dir.len())
        && if cfg!(windows) { p[..dir.len()].eq_ignore_ascii_case(dir) } else { &p[..dir.len()] == dir }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{manifest, stores};

    fn touch(p: &Path) {
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, "x").unwrap();
    }

    /// Emulador portátil achado pelo exe, pasta custom no config e save ao lado da ROM.
    #[test]
    fn emulators_in_roots() {
        let t = std::env::temp_dir().join(format!("ugs-emu-{}", std::process::id()));
        let _ = fs::remove_dir_all(&t);
        let dolphin = t.join("Emus/Dolphin-x64");
        touch(&dolphin.join("Dolphin.exe"));
        touch(&dolphin.join("User/GC/USA/Card A/01-GALE.gci"));
        touch(&dolphin.join("User/Cache/shaders.bin")); // não pode entrar
        let ra = t.join("Emus/RetroArch");
        let custom = t.join("Saves RA");
        touch(&ra.join("retroarch.exe"));
        fs::write(ra.join("retroarch.cfg"), format!("savefile_directory = \"{}\"\nsavestate_directory = \"default\"\n", custom.display())).unwrap();
        touch(&custom.join("Snes9x/Zelda.srm"));
        touch(&custom.join("Snes9x/Zelda.sfc")); // ROM: não pode entrar
        touch(&t.join("ROMs/GBA/Pokemon.gba"));
        touch(&t.join("ROMs/GBA/Pokemon.sav"));
        touch(&t.join("ROMs/GBA/Orfao.sav")); // sem ROM: não entra

        let m = manifest::with_extras(Manifest::new()).unwrap();
        let env = Env { vars: vec![], user: "x".into() };
        let st = stores::detect(&[t.clone()], &m);
        let names = |game: &str| -> Vec<String> {
            let (n, g) = find(&m, game).unwrap();
            // só o que está na pasta do teste (regras com caminho fixo acham o PC real)
            resolve(n, g, &env, &st)
                .files
                .iter()
                .filter(|p| p.starts_with(&t))
                .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
                .collect()
        };
        assert_eq!(names("Dolphin"), ["01-GALE.gci"]);
        assert_eq!(names("RetroArch"), ["Zelda.srm"]);
        assert_eq!(names("mGBA"), ["Pokemon.sav"]);
        assert_eq!(resolve("Dolphin", &m["Dolphin"], &env, &st).base.as_deref(), Some(dolphin.as_path()));
        fs::remove_dir_all(&t).unwrap();
    }

    /// Jogo Windows no Linux: regras de Windows resolvidas dentro do prefixo Proton/Heroic/Lutris.
    #[test]
    fn wine_prefixes() {
        use stores::{Installed, Store, Stores};
        let t = std::env::temp_dir().join(format!("ugs-wine-{}", std::process::id()));
        let _ = fs::remove_dir_all(&t);
        let m = manifest::parse(
            "Proton Game:\n  steam: {id: 123}\n  files:\n    \"<winAppData>/TestCo/*.dat\": {when: [{os: windows}]}\n\
             Heroic Game:\n  gog: {id: 456}\n  files:\n    \"<winDocuments>/My Games/HG/*.sav\": {}\n",
        )
        .unwrap();
        let env = Env { vars: vec![], user: "walter".into() };

        // Steam + Proton: compatdata/<id>/pfx, usuário "steamuser"
        let lib = t.join("lib");
        let pfx = lib.join("steamapps/compatdata/123/pfx");
        touch(&pfx.join("drive_c/users/steamuser/AppData/Roaming/TestCo/slot.dat"));
        let game_dir = lib.join("steamapps/common/ProtonGame");
        fs::create_dir_all(&game_dir).unwrap();
        assert_eq!(stores::proton_prefix(&lib, 123).as_deref(), Some(pfx.as_path()));
        let st = Stores {
            games: vec![Installed { steam_id: Some(123), prefix: Some(pfx.clone()), ..Installed::new(Store::Steam, game_dir) }],
            ..Default::default()
        };
        let f = resolve("Proton Game", &m["Proton Game"], &env, &st);
        assert_eq!(f.files.len(), 1, "{:?}", f.files);

        // Heroic (GOG) com prefixo Proton (`<winePrefix>/pfx`), usuário do sistema
        let heroic = t.join("heroic");
        let hpfx = t.join("Prefixes/HG");
        touch(&hpfx.join("pfx/drive_c/users/walter/Documents/My Games/HG/save1.sav"));
        let install = t.join("Games/HG");
        fs::create_dir_all(&install).unwrap();
        let write_json = |p: PathBuf, v: serde_json::Value| {
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, v.to_string()).unwrap();
        };
        write_json(heroic.join("gog_store/installed.json"), serde_json::json!({"installed": [{"appName": "456", "install_path": install}]}));
        write_json(heroic.join("GamesConfig/456.json"), serde_json::json!({"456": {"winePrefix": hpfx}}));
        let mut st = Stores::default();
        stores::heroic(&heroic, &mut st);
        assert_eq!(st.games.len(), 1);
        assert_eq!(st.games[0].gog_id, Some(456));
        assert_eq!(st.games[0].prefix.as_deref(), Some(hpfx.join("pfx").as_path()));
        assert_eq!(resolve("Heroic Game", &m["Heroic Game"], &env, &st).files.len(), 1);

        // Lutris: game.exe + game.prefix
        let lpfx = t.join("lutris-pfx");
        fs::create_dir_all(lpfx.join("drive_c")).unwrap();
        let exe = lpfx.join("drive_c/Games/LG/lg.exe");
        touch(&exe);
        let games = t.join("lutris-games");
        fs::create_dir_all(&games).unwrap();
        fs::write(games.join("lg-1.yml"), format!("game:\n  exe: '{}'\n  prefix: '{}'\n", exe.display(), lpfx.display())).unwrap();
        let mut st = Stores::default();
        stores::lutris(&games, &mut st);
        assert_eq!(st.games.len(), 1);
        assert_eq!(st.games[0].folder(), "LG");
        assert_eq!(st.games[0].prefix.as_deref(), Some(lpfx.as_path()));

        fs::remove_dir_all(&t).unwrap();
    }
}
