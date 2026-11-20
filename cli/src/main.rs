use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use ugs_core::{
    archive::{self, Outcome},
    manifest,
    scan::{self, Env},
    stores,
};

#[derive(Parser)]
#[command(name = "ugs", version, about = "Ultimate Game Saver — backup e restauração de saves")]
struct Cli {
    /// Pasta dos backups (padrão: ~/Ultimate Game Saver)
    #[arg(long, global = true, env = "UGS_BACKUP_DIR")]
    dir: Option<PathBuf>,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Baixa/atualiza o manifesto de jogos (Ludusavi/PCGamingWiki)
    UpdateManifest,
    /// Lista jogos com saves encontrados neste PC
    Scan { filter: Option<String> },
    /// Faz backup dos jogos indicados (nome exato) ou de todos
    Backup {
        games: Vec<String>,
        #[arg(long)]
        all: bool,
    },
    /// Restaura o último backup (ou --snapshot) — guarda antes o estado atual
    Restore {
        game: String,
        #[arg(long)]
        snapshot: Option<String>,
        /// Restaurar mesmo com o jogo aberto
        #[arg(long)]
        force: bool,
    },
    /// Lista os snapshots de um jogo
    Snapshots { game: String },
    /// Pastas raiz onde procurar emuladores portáteis, ports e ROMs
    Root {
        #[command(subcommand)]
        action: Option<RootCmd>,
    },
}

#[derive(Subcommand)]
enum RootCmd {
    /// Adiciona uma pasta
    Add { dir: PathBuf },
    /// Remove uma pasta
    Remove { dir: PathBuf },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let root = match cli.dir {
        Some(d) => d,
        None => dirs::home_dir().context("pasta do usuário não encontrada")?.join("Ultimate Game Saver"),
    };
    let data = dirs::data_local_dir().context("pasta de dados não encontrada")?.join("UltimateGameSaver");
    let manifest_path = data.join("manifest.yaml");
    let roots_path = data.join("roots.txt");
    let roots: Vec<PathBuf> =
        std::fs::read_to_string(&roots_path).unwrap_or_default().lines().filter(|l| !l.trim().is_empty()).map(PathBuf::from).collect();
    let detect = |m: &manifest::Manifest| stores::detect(&roots, m);

    match cli.cmd {
        Cmd::Root { action } => {
            let mut list = roots.clone();
            match action {
                None => {}
                Some(RootCmd::Add { dir }) => {
                    let dir = std::fs::canonicalize(&dir).with_context(|| format!("pasta não existe: {}", dir.display()))?;
                    // canonicalize no Windows devolve \\?\C:\...
                    let dir = PathBuf::from(dir.to_string_lossy().trim_start_matches(r"\\?\"));
                    if !list.contains(&dir) {
                        list.push(dir);
                    }
                }
                Some(RootCmd::Remove { dir }) => list.retain(|r| r != &dir),
            }
            std::fs::create_dir_all(&data)?;
            std::fs::write(&roots_path, list.iter().map(|r| format!("{}\n", r.display())).collect::<String>())?;
            for r in &list {
                println!("{}", r.display());
            }
            if list.is_empty() {
                println!("Nenhuma pasta raiz. Use `ugs root add <pasta>`.");
            }
        }
        Cmd::UpdateManifest => {
            let changed = manifest::update(&manifest_path)?;
            println!("{}", if changed { "Manifesto atualizado." } else { "Manifesto já está em dia." });
        }
        Cmd::Scan { filter } => {
            let m = manifest::load(&manifest_path)?;
            let found = scan::scan(&m, &Env::detect(), &detect(&m), filter.as_deref());
            for f in &found {
                let size: u64 = f.files.iter().filter_map(|p| p.metadata().ok()).map(|m| m.len()).sum();
                let reg = if f.registry.is_empty() { String::new() } else { format!(", {} chave(s) de registro", f.registry.len()) };
                println!("{}  —  {} arquivo(s), {}{reg}", f.name, f.files.len(), human(size));
            }
            println!("\n{} jogo(s) com saves.", found.len());
        }
        Cmd::Backup { games, all } => {
            if games.is_empty() && !all {
                bail!("indique jogos ou use --all");
            }
            let m = manifest::load(&manifest_path)?;
            let (env, st) = (Env::detect(), detect(&m));
            let found = if all {
                scan::scan(&m, &env, &st, None)
            } else {
                games
                    .iter()
                    .map(|g| {
                        let (n, game) = scan::find(&m, g).with_context(|| format!("jogo não está no manifesto: {g}"))?;
                        Ok(scan::resolve(n, game, &env, &st))
                    })
                    .collect::<Result<_>>()?
            };
            let mut failed = 0;
            for f in &found {
                match archive::backup(f, &env, &root, "") {
                    Ok(Outcome::Created(p)) => println!("✔ {}: {}", f.name, p.display()),
                    Ok(Outcome::Unchanged) => println!("= {}: sem mudanças", f.name),
                    Ok(Outcome::Nothing) => println!("– {}: nenhum save encontrado", f.name),
                    Err(e) => {
                        failed += 1;
                        eprintln!("✘ {}: {e:#}", f.name);
                    }
                }
            }
            if failed > 0 {
                bail!("{failed} backup(s) falharam");
            }
        }
        Cmd::Restore { game, snapshot, force } => {
            let m = manifest::load(&manifest_path)?;
            let env = Env::detect();
            let (n, g) = scan::find(&m, &game).with_context(|| format!("jogo não está no manifesto: {game}"))?;
            let current = scan::resolve(n, g, &env, &detect(&m));
            if let (Some(base), false) = (&current.base, force) {
                let running = archive::running_under(base);
                if !running.is_empty() {
                    bail!("feche o jogo antes de restaurar (aberto: {}). Ou use --force.", running.join(", "));
                }
            }
            let r = archive::restore(&current, &env, &root, snapshot.as_deref())?;
            println!(
                "✔ {}: {} arquivo(s) e {} chave(s) restaurados de {}",
                current.name,
                r.files,
                r.registry,
                r.snapshot.file_name().unwrap().to_string_lossy()
            );
            if let Some(s) = r.safety {
                let s = s.file_name().unwrap().to_string_lossy().into_owned();
                println!("  desfazer: ugs restore \"{}\" --snapshot {s}", current.name);
            }
        }
        Cmd::Snapshots { game } => {
            for s in archive::snapshots(&root, &game)? {
                let size = s.metadata().map(|m| m.len()).unwrap_or(0);
                println!("{}  ({})", s.file_name().unwrap().to_string_lossy(), human(size));
            }
        }
    }
    Ok(())
}

fn human(b: u64) -> String {
    match b {
        b if b >= 1 << 30 => format!("{:.1} GB", b as f64 / (1u64 << 30) as f64),
        b if b >= 1 << 20 => format!("{:.1} MB", b as f64 / (1u64 << 20) as f64),
        b if b >= 1 << 10 => format!("{:.1} KB", b as f64 / 1024.0),
        b => format!("{b} B"),
    }
}
