//! Snapshots em zip + `mapping.yaml`, e restauração com backup de segurança.
//!
//! Layout: `<raiz>/<jogo>/<AAAA-MM-DDTHH-MM-SSZ>[-pre-restore].zip`

use crate::scan::{is_under, norm, Env, Found};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

/// Snapshots mantidos por jogo.
pub const KEEP: usize = 10;
pub const PRE_RESTORE: &str = "-pre-restore";

#[derive(Serialize, Deserialize, Default)]
pub struct Mapping {
    pub game: String,
    pub created: String,
    pub files: Vec<Entry>,
    pub registry: Vec<Entry>,
}

/// `path`: caminho com placeholder (`<winAppData>/Jogo/save.dat`) ou chave de registro.
#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct Entry {
    pub path: String,
    pub sha256: String,
    pub size: u64,
}

pub enum Outcome {
    Created(PathBuf),
    Unchanged,
    Nothing,
}

pub struct Restored {
    pub snapshot: PathBuf,
    pub files: usize,
    pub registry: usize,
    pub safety: Option<PathBuf>,
}

pub fn game_dir(root: &Path, game: &str) -> PathBuf {
    let safe: String =
        game.chars().map(|c| if r#"<>:"/\|?*"#.contains(c) || c.is_control() { '_' } else { c }).collect();
    root.join(safe.trim_end_matches(['.', ' ']))
}

/// Snapshots do jogo, do mais antigo ao mais novo.
pub fn snapshots(root: &Path, game: &str) -> Result<Vec<PathBuf>> {
    let dir = game_dir(root, game);
    if !dir.is_dir() {
        return Ok(vec![]);
    }
    let mut v: Vec<PathBuf> = fs::read_dir(dir)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "zip"))
        .collect();
    v.sort();
    Ok(v)
}

pub fn read_mapping(snapshot: &Path) -> Result<Mapping> {
    let mut zip = ZipArchive::new(fs::File::open(snapshot)?)?;
    let m = serde_yaml::from_reader(zip.by_name("mapping.yaml")?)?;
    Ok(m)
}

pub fn backup(found: &Found, env: &Env, root: &Path, suffix: &str) -> Result<Outcome> {
    if found.files.is_empty() && found.registry.is_empty() {
        return Ok(Outcome::Nothing);
    }
    let mut mapping = Mapping { game: found.name.clone(), created: stamp(), ..Mapping::default() };
    for f in &found.files {
        let mut h = HashWriter::default();
        io::copy(&mut open(f)?, &mut h).with_context(|| format!("não consegui ler {}", f.display()))?;
        mapping.files.push(Entry { path: generalize(f, env, found.base.as_deref()), sha256: h.hex(), size: h.size });
    }
    let mut reg_data = Vec::new();
    for key in &found.registry {
        let bytes = reg_export(key)?;
        mapping.registry.push(entry(key.clone(), &bytes));
        reg_data.push(bytes);
    }

    let existing = snapshots(root, &found.name)?;
    if let Some(last) = existing.last() {
        let prev = read_mapping(last)?;
        if prev.files == mapping.files && prev.registry == mapping.registry {
            // data do zip = "conferido em", para a UI não marcar como alterado após um restore
            let _ = fs::File::options().append(true).open(last).and_then(|f| f.set_modified(SystemTime::now()));
            return Ok(Outcome::Unchanged);
        }
    }

    let dir = game_dir(root, &found.name);
    fs::create_dir_all(&dir)?;
    let mut path = dir.join(format!("{}{suffix}.zip", mapping.created));
    for n in 1.. {
        if !path.exists() {
            break;
        }
        path = dir.join(format!("{}{suffix}-{n}.zip", mapping.created));
    }
    let tmp = path.with_extension("zip.tmp");
    let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let mut z = ZipWriter::new(fs::File::create(&tmp)?);
    z.start_file("mapping.yaml", opts)?;
    z.write_all(serde_yaml::to_string(&mapping)?.as_bytes())?;
    for (e, f) in mapping.files.iter().zip(&found.files) {
        z.start_file(file_entry_name(&e.path), opts.large_file(e.size >= u32::MAX as u64))?;
        let mut h = HashWriter::default();
        io::copy(&mut open(f)?, &mut Tee(&mut z, &mut h))?;
        if h.hex() != e.sha256 {
            drop(z);
            let _ = fs::remove_file(&tmp);
            bail!("{} mudou durante o backup (jogo aberto?)", f.display());
        }
    }
    for (i, bytes) in reg_data.iter().enumerate() {
        z.start_file(reg_entry_name(i), opts)?;
        z.write_all(bytes)?;
    }
    z.finish()?.sync_all()?;
    fs::rename(&tmp, &path)?;

    let all = snapshots(root, &found.name)?;
    for old in &all[..all.len().saturating_sub(KEEP)] {
        fs::remove_file(old)?;
    }
    Ok(Outcome::Created(path))
}

/// Restaura `snapshot` (nome do arquivo, com ou sem `.zip`) ou o último backup normal.
/// Antes de escrever: valida todos os hashes e guarda o estado atual num snapshot `-pre-restore`.
pub fn restore(current: &Found, env: &Env, root: &Path, snapshot: Option<&str>) -> Result<Restored> {
    let snaps = snapshots(root, &current.name)?;
    let path = match snapshot {
        Some(s) => snaps.iter().find(|p| {
            let n = p.file_name().unwrap().to_string_lossy();
            n == s || n.trim_end_matches(".zip") == s
        }),
        None => snaps.iter().rev().find(|p| !p.to_string_lossy().contains(PRE_RESTORE)),
    }
    .context("nenhum snapshot encontrado")?
    .clone();

    let mut zip = ZipArchive::new(fs::File::open(&path)?)?;
    let mapping: Mapping = serde_yaml::from_reader(zip.by_name("mapping.yaml")?)?;

    // 1. extrair tudo para `<destino>.ugs-tmp` e conferir hashes — originais intactos até aqui
    let mut staged: Vec<(PathBuf, PathBuf)> = Vec::new();
    let mut regs = Vec::new();
    let result = (|| -> Result<()> {
        for e in &mapping.files {
            if e.path.split('/').any(|c| c == "..") {
                bail!("caminho inválido no backup: {}", e.path);
            }
            let dest = expand(&e.path, env, current.base.as_deref())?;
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut tmp = dest.clone().into_os_string();
            tmp.push(".ugs-tmp");
            let tmp = PathBuf::from(tmp);
            staged.push((tmp.clone(), dest));
            let mut h = HashWriter::default();
            io::copy(&mut zip.by_name(&file_entry_name(&e.path))?, &mut Tee(&mut fs::File::create(&tmp)?, &mut h))?;
            if h.hex() != e.sha256 {
                bail!("arquivo corrompido no backup: {}", e.path);
            }
        }
        for (i, e) in mapping.registry.iter().enumerate() {
            let mut bytes = Vec::new();
            zip.by_name(&reg_entry_name(i))?.read_to_end(&mut bytes)?;
            if sha(&bytes) != e.sha256 {
                bail!("arquivo corrompido no backup: {}", e.path);
            }
            regs.push(bytes);
        }
        Ok(())
    })();
    let cleanup = |staged: &[(PathBuf, PathBuf)]| staged.iter().for_each(|(t, _)| drop(fs::remove_file(t)));
    if let Err(e) = result {
        cleanup(&staged);
        return Err(e);
    }

    // 2. guardar o estado atual
    let safety = match backup(current, env, root, PRE_RESTORE) {
        Ok(Outcome::Created(p)) => Some(p),
        Ok(_) => None,
        Err(e) => {
            cleanup(&staged);
            return Err(e.context("backup de segurança falhou; nada foi restaurado"));
        }
    };

    // 3. trocar
    for (tmp, dest) in &staged {
        fs::rename(tmp, dest).with_context(|| format!("não consegui escrever {}", dest.display()))?;
    }
    for bytes in &regs {
        reg_import(bytes)?;
    }
    Ok(Restored { snapshot: path, files: staged.len(), registry: regs.len(), safety })
}

/// Executáveis em execução dentro de `dir` (jogo aberto = não restaurar).
pub fn running_under(dir: &Path) -> Vec<String> {
    let dir = norm(dir);
    running_exes().into_iter().filter(|e| is_under(e, &dir)).collect()
}

/// Caminhos (com `/`) dos executáveis em execução.
pub fn running_exes() -> Vec<String> {
    let mut sys = sysinfo::System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    sys.processes().values().filter_map(|p| p.exe()).map(norm).collect()
}

/// Nomes dos jogos que têm snapshots em `root` (inclusive os sem saves no PC agora).
pub fn backed_up_games(root: &Path) -> Vec<String> {
    let Ok(rd) = fs::read_dir(root) else { return vec![] };
    rd.flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| {
            let mut v: Vec<PathBuf> = fs::read_dir(e.path()).ok()?.flatten().map(|x| x.path()).filter(|p| p.extension().is_some_and(|x| x == "zip")).collect();
            v.sort();
            read_mapping(v.last()?).ok().map(|m| m.game)
        })
        .collect()
}

fn open(p: &Path) -> Result<fs::File> {
    fs::File::open(p).with_context(|| format!("não consegui abrir {}", p.display()))
}

fn entry(path: String, bytes: &[u8]) -> Entry {
    Entry { path, sha256: sha(bytes), size: bytes.len() as u64 }
}

#[derive(Default)]
struct HashWriter {
    h: Sha256,
    size: u64,
}

impl HashWriter {
    fn hex(&self) -> String {
        self.h.clone().finalize().iter().map(|x| format!("{x:02x}")).collect()
    }
}

impl Write for HashWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.h.update(buf);
        self.size += buf.len() as u64;
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Escreve em `0` e soma o hash em `1`.
struct Tee<'a, W: Write>(&'a mut W, &'a mut HashWriter);

impl<W: Write> Write for Tee<'_, W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let n = self.0.write(buf)?;
        self.1.write_all(&buf[..n])?;
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

fn file_entry_name(generalized: &str) -> String {
    format!("files/{}", generalized.replace(['<', '>', ':'], ""))
}

fn reg_entry_name(i: usize) -> String {
    format!("registry/{i}.reg")
}

fn sha(b: &[u8]) -> String {
    Sha256::digest(b).iter().map(|x| format!("{x:02x}")).collect()
}

/// Troca o prefixo mais longo conhecido por placeholder, para restaurar em outro PC/usuário.
pub fn generalize(path: &Path, env: &Env, base: Option<&Path>) -> String {
    let p = norm(path);
    let base = base.map(norm);
    let best = env
        .vars
        .iter()
        .map(|(k, v)| (*k, v.as_str()))
        .chain(base.as_deref().map(|b| ("<base>", b)))
        .filter(|(_, v)| is_under(&p, v))
        .max_by_key(|(_, v)| v.len());
    match best {
        Some((k, v)) => format!("{k}{}", &p[v.len()..]),
        None => p,
    }
}

pub fn expand(generalized: &str, env: &Env, base: Option<&Path>) -> Result<PathBuf> {
    if !generalized.starts_with('<') {
        return Ok(PathBuf::from(generalized));
    }
    let end = generalized.find('>').context("placeholder sem `>`")? + 1;
    let (ph, tail) = generalized.split_at(end);
    let value = if ph == "<base>" {
        norm(base.context("jogo não instalado: pasta <base> desconhecida")?)
    } else {
        env.get(ph).with_context(|| format!("{ph} não existe neste sistema"))?.to_string()
    };
    Ok(PathBuf::from(format!("{value}{tail}")))
}

fn stamp() -> String {
    let s = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let (y, m, d) = civil((s / 86400) as i64);
    let t = s % 86400;
    format!("{y:04}-{m:02}-{d:02}T{:02}-{:02}-{:02}Z", t / 3600, t / 60 % 60, t % 60)
}

/// Dias desde 1970-01-01 para (ano, mês, dia). Algoritmo de Howard Hinnant.
fn civil(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

#[cfg(windows)]
fn reg_export(key: &str) -> Result<Vec<u8>> {
    let tmp = std::env::temp_dir().join(format!("ugs-{}-{}.reg", std::process::id(), &sha(key.as_bytes())[..16]));
    let out = std::process::Command::new("reg")
        .args(["export", &key.replace('/', "\\")])
        .arg(&tmp)
        .arg("/y")
        .output()?;
    if !out.status.success() {
        bail!("reg export falhou: {key}");
    }
    let b = fs::read(&tmp)?;
    let _ = fs::remove_file(&tmp);
    Ok(b)
}

#[cfg(windows)]
fn reg_import(bytes: &[u8]) -> Result<()> {
    let tmp = std::env::temp_dir().join(format!("ugs-{}-{}.reg", std::process::id(), &sha(bytes)[..16]));
    fs::write(&tmp, bytes)?;
    let out = std::process::Command::new("reg").arg("import").arg(&tmp).output()?;
    let _ = fs::remove_file(&tmp);
    if !out.status.success() {
        bail!("reg import falhou");
    }
    Ok(())
}

#[cfg(not(windows))]
fn reg_export(_: &str) -> Result<Vec<u8>> {
    bail!("registro só existe no Windows")
}

#[cfg(not(windows))]
fn reg_import(_: &[u8]) -> Result<()> {
    bail!("registro só existe no Windows")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{manifest, scan, stores::Stores};

    #[test]
    fn civil_dates() {
        assert_eq!(civil(0), (1970, 1, 1));
        assert_eq!(civil(19_723), (2024, 1, 1));
        assert_eq!(civil(11_016), (2000, 2, 29));
    }

    /// backup → mexer nos saves → restore → conteúdo original; pre-restore desfaz.
    #[test]
    fn round_trip() {
        let tmp = std::env::temp_dir().join(format!("ugs-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        let saves = tmp.join("appdata/TestGame");
        let root = tmp.join("backups");
        fs::create_dir_all(&saves).unwrap();
        fs::write(saves.join("slot1.sav"), "original 1").unwrap();
        fs::write(saves.join("slot2.sav"), "original 2").unwrap();
        fs::write(saves.join("notes.txt"), "ignorar").unwrap();

        let env = Env { vars: vec![("<winAppData>", norm(&tmp.join("appdata")))], user: "x".into() };
        let m = manifest::parse("Test Game:\n  files:\n    \"<winAppData>/TestGame/*.sav\": {tags: [save]}\n").unwrap();
        let (name, game) = scan::find(&m, "test game").unwrap();
        let stores = Stores::default();
        let found = scan::resolve(name, game, &env, &stores);
        assert_eq!(found.files.len(), 2);

        assert!(matches!(backup(&found, &env, &root, "").unwrap(), Outcome::Created(_)));
        assert!(matches!(backup(&found, &env, &root, "").unwrap(), Outcome::Unchanged));
        let mapping = read_mapping(snapshots(&root, name).unwrap().last().unwrap()).unwrap();
        assert_eq!(mapping.files[0].path, "<winAppData>/TestGame/slot1.sav");

        fs::write(saves.join("slot1.sav"), "estragado").unwrap();
        fs::remove_file(saves.join("slot2.sav")).unwrap();
        let current = scan::resolve(name, game, &env, &stores);
        let r = restore(&current, &env, &root, None).unwrap();
        assert_eq!(r.files, 2);
        assert_eq!(fs::read_to_string(saves.join("slot1.sav")).unwrap(), "original 1");
        assert_eq!(fs::read_to_string(saves.join("slot2.sav")).unwrap(), "original 2");

        // desfazer: o snapshot de segurança tem o estado "estragado"
        let safety = r.safety.unwrap();
        let current = scan::resolve(name, game, &env, &stores);
        restore(&current, &env, &root, Some(&safety.file_name().unwrap().to_string_lossy())).unwrap();
        assert_eq!(fs::read_to_string(saves.join("slot1.sav")).unwrap(), "estragado");

        fs::remove_dir_all(&tmp).unwrap();
    }
}
