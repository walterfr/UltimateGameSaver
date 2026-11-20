//! Pastas do app: dados (manifesto, pastas raiz, preferências) e backups.

use anyhow::{Context, Result};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone)]
pub struct Paths {
    pub data: PathBuf,
}

impl Paths {
    pub fn new() -> Result<Self> {
        let data = dirs::data_local_dir().context("pasta de dados não encontrada")?.join("UltimateGameSaver");
        Ok(Paths { data })
    }

    pub fn manifest(&self) -> PathBuf {
        self.data.join("manifest.yaml")
    }

    /// Pastas raiz onde procurar emuladores portáteis, ports e ROMs.
    pub fn roots(&self) -> Vec<PathBuf> {
        fs::read_to_string(self.data.join("roots.txt"))
            .unwrap_or_default()
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(PathBuf::from)
            .collect()
    }

    pub fn add_root(&self, dir: &Path) -> Result<Vec<PathBuf>> {
        let dir = fs::canonicalize(dir).with_context(|| format!("pasta não existe: {}", dir.display()))?;
        // canonicalize no Windows devolve \\?\C:\...
        let dir = PathBuf::from(dir.to_string_lossy().trim_start_matches(r"\\?\"));
        let mut list = self.roots();
        if !list.contains(&dir) {
            list.push(dir);
        }
        self.write_roots(&list)?;
        Ok(list)
    }

    pub fn remove_root(&self, dir: &Path) -> Result<Vec<PathBuf>> {
        let mut list = self.roots();
        list.retain(|r| r != dir);
        self.write_roots(&list)?;
        Ok(list)
    }

    fn write_roots(&self, list: &[PathBuf]) -> Result<()> {
        fs::create_dir_all(&self.data)?;
        fs::write(self.data.join("roots.txt"), list.iter().map(|r| format!("{}\n", r.display())).collect::<String>())?;
        Ok(())
    }

    /// Pasta dos backups (padrão: ~/Ultimate Game Saver). Pode ficar no OneDrive/Drive para ter nuvem.
    pub fn backup_dir(&self) -> PathBuf {
        fs::read_to_string(self.data.join("backup_dir.txt"))
            .ok()
            .map(|s| PathBuf::from(s.trim()))
            .filter(|p| !p.as_os_str().is_empty())
            .or_else(|| dirs::home_dir().map(|h| h.join("Ultimate Game Saver")))
            .unwrap_or_else(|| PathBuf::from("Ultimate Game Saver"))
    }

    pub fn set_backup_dir(&self, dir: &Path) -> Result<()> {
        fs::create_dir_all(&self.data)?;
        fs::write(self.data.join("backup_dir.txt"), dir.to_string_lossy().as_bytes())?;
        Ok(())
    }
}
