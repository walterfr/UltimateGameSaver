#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use anyhow::{bail, Context, Result};
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, WindowEvent,
};
use tauri_plugin_dialog::DialogExt;
use ugs_core::{
    archive::{self, Outcome},
    config::Paths,
    manifest::{self, Manifest},
    scan::{self, is_under, norm, Env, Found},
    stores::{self, Stores},
};

/// Espera após o jogo fechar antes do backup automático (jogo termina de gravar).
const AFTER_EXIT: Duration = Duration::from_secs(10);

struct AppState {
    paths: Paths,
    manifest: Mutex<Option<Arc<Manifest>>>,
    found: Mutex<Arc<Vec<Found>>>,
    stores: Mutex<Arc<Stores>>,
}

fn state(app: &AppHandle) -> tauri::State<'_, AppState> {
    app.state::<AppState>()
}

fn load_manifest(app: &AppHandle) -> Result<Arc<Manifest>> {
    let st = state(app);
    let mut m = st.manifest.lock().unwrap();
    if m.is_none() {
        let path = st.paths.manifest();
        if !path.exists() {
            manifest::update(&path)?;
        }
        *m = Some(Arc::new(manifest::load(&path)?));
    }
    Ok(m.clone().unwrap())
}

/// Roda trabalho pesado fora da thread da UI.
async fn run<T: Send + 'static>(app: AppHandle, f: impl FnOnce(&AppHandle) -> Result<T> + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(move || f(&app)).await.map_err(|e| e.to_string())?.map_err(|e| format!("{e:#}"))
}

#[derive(Serialize)]
struct Row {
    name: String,
    files: usize,
    size: u64,
    registry: usize,
    last_backup: Option<u64>,
    changed: bool,
    on_pc: bool,
    watched: bool,
}

fn rows(app: &AppHandle) -> Vec<Row> {
    let st = state(app);
    let root = st.paths.backup_dir();
    let found = st.found.lock().unwrap().clone();
    let mut v: Vec<Row> = found
        .iter()
        .map(|f| {
            let metas: Vec<_> = f.files.iter().filter_map(|p| p.metadata().ok()).collect();
            let newest = metas.iter().filter_map(|m| m.modified().ok()).max();
            let last = last_backup(&root, &f.name);
            Row {
                name: f.name.clone(),
                files: f.files.len(),
                size: metas.iter().map(|m| m.len()).sum(),
                registry: f.registry.len(),
                last_backup: last.map(secs),
                // ponytail: compara datas, não hashes; mudança só no registro não aparece aqui
                changed: match (newest, last) {
                    (Some(n), Some(l)) => n > l,
                    (_, None) => true,
                    _ => false,
                },
                on_pc: true,
                watched: f.base.is_some(),
            }
        })
        .collect();
    let known: HashSet<String> = v.iter().map(|r| r.name.clone()).collect();
    for name in archive::backed_up_games(&root) {
        if !known.contains(&name) {
            v.push(Row {
                last_backup: last_backup(&root, &name).map(secs),
                name,
                files: 0,
                size: 0,
                registry: 0,
                changed: false,
                on_pc: false,
                watched: false,
            });
        }
    }
    v.sort_by_key(|r| r.name.to_lowercase());
    v
}

fn last_backup(root: &Path, game: &str) -> Option<SystemTime> {
    archive::snapshots(root, game).ok()?.last()?.metadata().ok()?.modified().ok()
}

fn secs(t: SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn file_name(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

fn scan_now(app: &AppHandle) -> Result<()> {
    let m = load_manifest(app)?;
    let st = state(app);
    let stores = Arc::new(stores::detect(&st.paths.roots(), &m));
    let found = scan::scan(&m, &Env::detect(), &stores, None);
    *st.stores.lock().unwrap() = stores;
    *st.found.lock().unwrap() = Arc::new(found);
    Ok(())
}

/// Backup com os arquivos resolvidos agora (pode haver saves novos desde o scan).
fn backup_games(app: &AppHandle, names: &[String]) -> Result<Vec<String>> {
    let m = load_manifest(app)?;
    let st = state(app);
    let (root, env, stores) = (st.paths.backup_dir(), Env::detect(), st.stores.lock().unwrap().clone());
    Ok(names
        .iter()
        .map(|name| {
            let Some((n, g)) = scan::find(&m, name) else { return format!("✘ {name}: não está no manifesto") };
            match archive::backup(&scan::resolve(n, g, &env, &stores), &env, &root, "") {
                Ok(Outcome::Created(_)) => format!("✔ {n}: backup criado"),
                Ok(Outcome::Unchanged) => format!("= {n}: sem mudanças"),
                Ok(Outcome::Nothing) => format!("– {n}: nenhum save encontrado"),
                Err(e) => format!("✘ {n}: {e:#}"),
            }
        })
        .collect())
}

fn backup_everything(app: &AppHandle) -> Result<Vec<String>> {
    if state(app).found.lock().unwrap().is_empty() {
        scan_now(app)?;
    }
    let names: Vec<String> = state(app).found.lock().unwrap().iter().map(|f| f.name.clone()).collect();
    backup_games(app, &names)
}

#[tauri::command]
async fn scan(app: AppHandle) -> Result<Vec<Row>, String> {
    run(app, |a| {
        scan_now(a)?;
        Ok(rows(a))
    })
    .await
}

/// Atualiza datas/status sem escanear de novo.
#[tauri::command]
async fn list(app: AppHandle) -> Result<Vec<Row>, String> {
    run(app, |a| Ok(rows(a))).await
}

#[tauri::command]
async fn backup(app: AppHandle, names: Vec<String>) -> Result<Vec<String>, String> {
    run(app, move |a| backup_games(a, &names)).await
}

#[tauri::command]
async fn backup_all(app: AppHandle) -> Result<Vec<String>, String> {
    run(app, backup_everything).await
}

#[derive(Serialize)]
struct Restored {
    message: String,
    undo: Option<String>,
}

#[tauri::command]
async fn restore(app: AppHandle, name: String, snapshot: Option<String>) -> Result<Restored, String> {
    run(app, move |a| {
        let m = load_manifest(a)?;
        let st = state(a);
        let (env, stores) = (Env::detect(), st.stores.lock().unwrap().clone());
        let (n, g) = scan::find(&m, &name).context("jogo não está no manifesto")?;
        let current = scan::resolve(n, g, &env, &stores);
        if let Some(base) = &current.base {
            let running = archive::running_under(base);
            if !running.is_empty() {
                bail!("feche o jogo antes de restaurar ({})", running.join(", "));
            }
        }
        let r = archive::restore(&current, &env, &st.paths.backup_dir(), snapshot.as_deref())?;
        Ok(Restored {
            message: format!(
                "{}: {} arquivo(s) e {} chave(s) restaurados de {}",
                n,
                r.files,
                r.registry,
                file_name(&r.snapshot)
            ),
            undo: r.safety.map(|p| file_name(&p)),
        })
    })
    .await
}

#[derive(Serialize)]
struct Snap {
    file: String,
    size: u64,
    pre_restore: bool,
}

#[tauri::command]
async fn snapshots(app: AppHandle, name: String) -> Result<Vec<Snap>, String> {
    run(app, move |a| {
        let mut v: Vec<Snap> = archive::snapshots(&state(a).paths.backup_dir(), &name)?
            .iter()
            .map(|p| Snap {
                file: file_name(p),
                size: p.metadata().map(|m| m.len()).unwrap_or(0),
                pre_restore: file_name(p).contains(archive::PRE_RESTORE),
            })
            .collect();
        v.reverse();
        Ok(v)
    })
    .await
}

#[derive(Serialize)]
struct Settings {
    roots: Vec<String>,
    backup_dir: String,
}

fn settings_of(app: &AppHandle) -> Settings {
    let p = &state(app).paths;
    Settings {
        roots: p.roots().iter().map(|r| r.display().to_string()).collect(),
        backup_dir: p.backup_dir().display().to_string(),
    }
}

fn pick_folder(app: &AppHandle) -> Option<std::path::PathBuf> {
    app.dialog().file().blocking_pick_folder()?.into_path().ok()
}

#[tauri::command]
async fn settings(app: AppHandle) -> Result<Settings, String> {
    run(app, |a| Ok(settings_of(a))).await
}

#[tauri::command]
async fn add_root(app: AppHandle) -> Result<Settings, String> {
    run(app, |a| {
        if let Some(dir) = pick_folder(a) {
            state(a).paths.add_root(&dir)?;
        }
        Ok(settings_of(a))
    })
    .await
}

#[tauri::command]
async fn remove_root(app: AppHandle, dir: String) -> Result<Settings, String> {
    run(app, move |a| {
        state(a).paths.remove_root(Path::new(&dir))?;
        Ok(settings_of(a))
    })
    .await
}

#[tauri::command]
async fn choose_backup_dir(app: AppHandle) -> Result<Settings, String> {
    run(app, |a| {
        if let Some(dir) = pick_folder(a) {
            state(a).paths.set_backup_dir(&dir)?;
        }
        Ok(settings_of(a))
    })
    .await
}

#[tauri::command]
async fn update_manifest(app: AppHandle) -> Result<String, String> {
    run(app, |a| {
        let st = state(a);
        let changed = manifest::update(&st.paths.manifest())?;
        *st.manifest.lock().unwrap() = None;
        Ok(if changed { "Manifesto atualizado." } else { "Manifesto já está em dia." }.to_string())
    })
    .await
}

/// Backup automático quando um jogo/emulador com pasta conhecida fecha.
fn monitor(app: AppHandle) {
    let mut running: HashSet<String> = HashSet::new();
    let mut closed: HashMap<String, Instant> = HashMap::new();
    loop {
        thread::sleep(Duration::from_secs(5));
        let found = state(&app).found.lock().unwrap().clone();
        let watch: Vec<(&str, String)> =
            found.iter().filter_map(|f| Some((f.name.as_str(), norm(f.base.as_ref()?)))).collect();
        if watch.is_empty() {
            continue;
        }
        let exes = archive::running_exes();
        let now: HashSet<String> =
            watch.iter().filter(|(_, b)| exes.iter().any(|e| is_under(e, b))).map(|(n, _)| n.to_string()).collect();
        for g in running.difference(&now) {
            closed.insert(g.clone(), Instant::now());
        }
        closed.retain(|g, _| !now.contains(g));
        running = now;
        let ready: Vec<String> = closed.iter().filter(|(_, t)| t.elapsed() >= AFTER_EXIT).map(|(g, _)| g.clone()).collect();
        for g in ready {
            closed.remove(&g);
            let msgs = backup_games(&app, &[g]).unwrap_or_else(|e| vec![format!("✘ {e:#}")]);
            let _ = app.emit("auto-backup", msgs);
        }
    }
}

fn show(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

fn tray(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Abrir", true, None::<&str>)?;
    let all = MenuItem::with_id(app, "backup_all", "Backup de tudo", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Sair", true, None::<&str>)?;
    TrayIconBuilder::new()
        .icon(app.default_window_icon().cloned().expect("ícone"))
        .tooltip("Ultimate Game Saver")
        .menu(&Menu::with_items(app, &[&open, &all, &quit])?)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, e| match e.id.as_ref() {
            "open" => show(app),
            "backup_all" => {
                let a = app.clone();
                thread::spawn(move || {
                    let msgs = backup_everything(&a).unwrap_or_else(|e| vec![format!("✘ {e:#}")]);
                    let _ = a.emit("auto-backup", msgs);
                });
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, e| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
                show(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

fn main() {
    let paths = Paths::new().expect("pasta de dados do usuário");
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            paths,
            manifest: Mutex::new(None),
            found: Mutex::default(),
            stores: Mutex::default(),
        })
        .invoke_handler(tauri::generate_handler![
            scan,
            list,
            backup,
            backup_all,
            restore,
            snapshots,
            settings,
            add_root,
            remove_root,
            choose_backup_dir,
            update_manifest
        ])
        .setup(|app| {
            tray(app.handle())?;
            let h = app.handle().clone();
            thread::spawn(move || monitor(h));
            Ok(())
        })
        // fechar a janela = ir para a bandeja (o backup automático continua)
        .on_window_event(|w, e| {
            if let WindowEvent::CloseRequested { api, .. } = e {
                api.prevent_close();
                let _ = w.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o app");
}
