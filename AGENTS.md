# AGENTS.md — Ultimate Game Saver

Guia para agentes de código (Antigravity, Claude Code, Codex, Cursor etc.) e pessoas que forem continuar este projeto.
Leia junto com [PLANO.md](PLANO.md), que tem a pesquisa completa de onde ficam os saves e o roadmap.

## O que é

Organizador de saves de jogos de PC e emuladores: acha os saves, faz backup versionado em zip e restaura com um clique.
Alvo: Windows (pronto), Linux (pronto, sem teste em máquina real) e Android (armazenamento compartilhado pronto). Repo: https://github.com/walterfr/UltimateGameSaver

## Estado atual

| Fase | Status |
|---|---|
| 1. Motor + CLI no Windows (manifesto, Steam/Epic/GOG, backup/restore) | ✔ |
| 2. Linux (Steam/Flatpak, Proton, Heroic, Lutris, AppImage/deb) | ✔ no código e no CI — **falta testar em máquina Linux real** |
| 3. Emuladores e ports (`core/data/extras.yaml`) | ✔ |
| 4. App desktop Tauri 2 + instalador NSIS | ✔ — Release v0.1.0 publicada (só Windows, build local) |
| Release automática (AppImage, .deb, NSIS pelo CI) | ✔ — Release v0.2.0 publicada com AppImage (79 MB), .deb (4,6 MB) e .exe (2,9 MB) gerados pelo `release.yml` |
| 5. Android (armazenamento compartilhado) | ✔ — testado num Lenovo Tab (Android 16): acha, faz backup e restaura saves de RetroArch e PPSSPP. Falta: `Android/data` (Shizuku), APK assinado na release |
| 6. Remapeamento de IDs entre PCs/contas, sync PC ↔ Android | pendente |

## Estrutura

```
Cargo.toml              workspace: core, cli, app/src-tauri
core/                   crate `ugs-core` (lib) — toda a lógica
  data/extras.yaml      regras próprias de emuladores e ports (embutido via include_str!)
  src/manifest.rs       tipos do manifesto Ludusavi, download (ETag), merge com extras.yaml
  src/stores.rs         detecção de Steam/Epic/GOG + varredura de pastas raiz (exe, saves ao lado da ROM)
  src/scan.rs           Env (placeholders), contextos por loja, expansão de padrões, glob, leitura de config
  src/archive.rs        backup (zip + mapping.yaml), restore, snapshots, processos em execução
  src/config.rs         Paths: pasta de dados, pastas raiz, pasta dos backups
cli/                    crate `ugs` (binário de linha de comando)
app/src-tauri/          crate `ugs-app` (Tauri 2): comandos, bandeja, monitor de processos
  src/lib.rs            todo o app; `run_app()` é a entrada (mobile_entry_point no Android)
  src/main.rs           só chama `ugs_app_lib::run_app()` no desktop
  package.json          fixa a CLI do Tauri (o Gradle do Android chama `npm run tauri`)
  gen/android/          projeto Android gerado por `tauri android init` — versionado e editado à mão
                        (MainActivity.kt pede "acesso a todos os arquivos"; AndroidManifest.xml tem as permissões)
app/ui/                 frontend HTML/CSS/JS puro, sem bundler
PLANO.md                pesquisa + plano + roadmap
```

## Comandos

```powershell
cargo test                                   # testes (core: round-trip backup/restore, emuladores, datas, VDF)
cargo run -p ugs -- update-manifest          # baixa o manifesto (~18 MB)
cargo run -p ugs -- scan [filtro]            # lista jogos com saves
cargo run -p ugs -- backup "Nome" | --all    # nomes exatos do manifesto
cargo run -p ugs -- restore "Nome" [--snapshot ARQ] [--force]
cargo run -p ugs -- snapshots "Nome"
cargo run -p ugs -- root add <pasta>         # pastas raiz (emuladores portáteis, ports, ROMs)
cargo run -p ugs-app                         # app desktop
cd app/src-tauri; npx @tauri-apps/cli@2 build   # instalador: target/release/bundle/nsis/*.exe
```

Use `--dir <pasta>` ou a variável `UGS_BACKUP_DIR` para a pasta de backup. **Durante o desenvolvimento, nunca faça backup/restore na pasta real do usuário** — use uma pasta temporária.

## Arquivos em tempo de execução

- `%LOCALAPPDATA%\UltimateGameSaver\` (Linux: `~/.local/share/UltimateGameSaver/`)
  - `manifest.yaml` + `manifest.etag` — cache do manifesto
  - `roots.txt` — pastas raiz, uma por linha
  - `backup_dir.txt` — pasta de backup escolhida (se ausente: `~/Ultimate Game Saver`)
- Backups: `<pasta de backup>/<nome do jogo sanitizado>/<AAAA-MM-DDTHH-MM-SSZ>[-pre-restore][-N].zip`
  - `mapping.yaml` — jogo, data, e para cada arquivo: caminho generalizado, sha256, tamanho
  - `files/<caminho sem < > :>` — conteúdo
  - `registry/<i>.reg` — chaves HKCU exportadas com `reg export`
  - Mantém os 10 snapshots mais novos por jogo (`archive::KEEP`)

## Conceitos-chave

### Manifesto + extras
- Fonte principal: manifesto do [Ludusavi](https://github.com/mtkennerly/ludusavi-manifest) (dados do PCGamingWiki). Formato: `Nome: { files: {padrão: {tags, when: [{os, store}]}}, registry, installDir, launch, steam: {id}, gog: {id} }`.
- `core/data/extras.yaml` usa o mesmo formato e é aplicado por cima (`manifest::with_extras`, chamado por `manifest::load`):
  - `files` — se presente, **substitui** os files da entrada (ids/installDir/launch ficam)
  - `exe` — caminho do executável relativo a `<base>`, procurado nas pastas raiz
  - `configDirs` — `arquivo de config → chave → padrões` dentro da pasta que a chave aponta (`chave = valor`, `chave: valor` ou `<chave>valor</chave>`; valor `:\x` = relativo ao config; `default`/vazio = ignorado)
  - `sidecar` — `{roms: [ext], saves: [ext]}`: saves com o mesmo nome de uma ROM na mesma pasta
  - Âncoras YAML (`&x` / `*x`) são usadas para repetir blocos de configDirs.
- Regra de ouro das entradas de emulador: **só saves e states**. Nada de cache, shaders, texturas, firmware, jogos instalados (ex.: 3DS só `title/*/*/data` e `extdata`; Xenia só `00000001`; Wii só `title/00010000`).
- Entradas não confirmadas na prática estão marcadas "(verificar)" no PLANO.md.

### Placeholders
`<base>` pasta de instalação · `<root>` raiz da loja · `<game>` nome da pasta · `<storeUserId>` (vira `*`) · `<storeGameId>` · `<osUserName>` · `<home>` · Windows: `<winAppData>`, `<winLocalAppData>`, `<winLocalAppDataLow>`, `<winDocuments>`, `<winPublic>`, `<winProgramData>`, `<winDir>` · Linux: `<xdgData>`, `<xdgConfig>`.
- Valores substituídos são escapados para glob (`Pattern::escape`); placeholder sem valor = padrão ignorado.
- No backup, o caminho absoluto é **generalizado** de volta (prefixo conhecido mais longo vira placeholder) para restaurar em outro PC/usuário (`archive::generalize` / `archive::expand`).

### Contextos (scan.rs `contexts`)
Cada jogo é procurado em: cada instalação que casa (Steam id, GOG id, nome da pasta de `installDir`, ou jogo achado por exe nas pastas raiz) + contexto Steam sem instalação (saves do Steam Cloud em `userdata`) + contexto genérico (sem `<base>`). `when.store` é tolerante quando a loja é desconhecida; a existência no disco filtra.

### Wine/Proton (Linux)
- `Installed.prefix` = pasta que contém `drive_c`. Origens: Steam `steamapps/compatdata/<AppID>/pfx` (`stores::proton_prefix`), Heroic `GamesConfig/<app>.json → winePrefix` (ou `winePrefix/pfx` com Proton), Lutris `game.prefix` no `.yml`.
- Um contexto com prefixo tem `vars` próprias (`wine_vars`): `<winAppData>` → `drive_c/users/<usuário>/AppData/Roaming` etc.; usuário = o do sistema ou `steamuser`. Nesse contexto as regras `when: os: windows` valem e o glob ignora maiúsculas.
- Heroic: `~/.config/heroic` e Flatpak; GOG em `gog_store/installed.json` (appName = id GOG), Epic via `legendaryConfig/legendary/installed.json`. Lutris: `~/.config/lutris/games/*.yml`, `~/.local/share/lutris/games/` e Flatpak; `<base>` = pasta do `game.exe`.
- ponytail: saves dentro de prefixo são generalizados como caminho Linux (`<home>/.local/share/Steam/...`), então restaurar um save de Windows dentro do Proton (ou o contrário) ainda não funciona — é a Fase 6. Registro do Wine (`user.reg`) não entra no backup.

### Invariantes de segurança (não quebrar)
- **Backup**: arquivos lidos em streaming com hash; se o hash mudar entre a 1ª e a 2ª leitura, aborta ("mudou durante o backup"). Escreve `.zip.tmp` e renomeia. Se nada mudou, não cria snapshot e só atualiza a data do último zip (a UI usa essa data como "conferido em").
- **Restore**: (1) extrai tudo para `<destino>.ugs-tmp` conferindo sha256 — originais intactos; (2) faz snapshot `-pre-restore` do estado atual; (3) só então renomeia. Qualquer erro antes do passo 3 apaga os temporários. Caminhos com `..` no mapping são rejeitados.
- Restore recusa se houver processo rodando dentro de `<base>` (a não ser com `--force`).
- Restaurar sem `--snapshot` usa o último snapshot **normal** (ignora `-pre-restore`).
- Registro: só `HKEY_CURRENT_USER` (HKLM exige admin para importar).
- `manifest::update` valida o YAML baixado antes de substituir o cache.

### App (Tauri 2)
- `tauri` com feature `custom-protocol` fixa: `ui/` é embutida, então `cargo run -p ugs-app` funciona sem a CLI do Tauri.
- `withGlobalTauri: true`; o JS usa `window.__TAURI__.core.invoke`. Argumentos de comandos são de uma palavra (Tauri converte snake_case → camelCase).
- Todo comando é `async` e roda o trabalho em `spawn_blocking` (função `run`), para não travar a UI.
- Estado: manifesto em cache (`Arc`), último scan (`found`) e lojas detectadas (`stores`).
- Fechar a janela esconde (bandeja). Monitor: a cada 5 s vê processos; quando um jogo com `<base>` fecha, faz backup 10 s depois e emite o evento `auto-backup`.
- Status "Alterado" = algum arquivo com data mais nova que o último zip (não compara hashes).
- `app/src-tauri/gen/` é gerado — está no `.gitignore`.
- Ícones em `app/src-tauri/icons/` foram gerados com System.Drawing (ICO com PNG embutido).

## Convenções

- Texto para o usuário (UI, mensagens da CLI, erros, docs): **português do Brasil**. Identificadores de código em inglês; comentários em português.
- Mínimo de dependências e de código: preferir stdlib e recursos nativos (ex.: `reg.exe` para registro, seletor de pasta nativo, `<dialog>` HTML). Simplificações deliberadas levam comentário `ponytail:` dizendo o limite e como evoluir.
- Testes: um teste pequeno por lógica não trivial, sem frameworks. **Testes têm que ser herméticos** — regras com caminho fixo (ex.: `C:/RetroArch-Win64`) acham coisas reais da máquina; filtre resultados pela pasta temporária do teste.
- Commits no estilo Conventional Commits (`feat:`, `fix:`, `build:`, `docs:`), mensagem em português.
- Versão: alterar em `app/src-tauri/tauri.conf.json` e nos três `Cargo.toml` (atual: 0.2.0).

## Releases

- `.github/workflows/release.yml` roda quando uma release é **publicada** no GitHub: compila em `ubuntu-22.04` (AppImage + .deb; 22.04 pela glibc mais antiga) e `windows-latest` (NSIS), troca espaços do nome por `UltimateGameSaver_...` e anexa com `gh release upload --clobber`.
- Publicar: subir a versão, commit + push, depois
  `gh release create v<versão> --target main --title "Ultimate Game Saver <versão>" --notes-file <notas.md>`
  (sem anexos — o workflow anexa). Conferir em Actions → Release e `gh release view v<versão>`.
- Refazer anexos de uma release existente: Actions → Release → Run workflow → informar a tag.
- Notas em português; avisar que o instalador Windows **não é assinado** (SmartScreen) e que o Linux é experimental até ser testado em máquina real.
- Última release: **v0.2.0** (suporte a Linux), anexos gerados pelo workflow sem erro. Próxima versão: 0.3.0.

## Pegadinhas do ambiente (Windows)

- Shell padrão é PowerShell. `git commit -F -` com here-string não funciona — escreva a mensagem num arquivo e use `-F arquivo`.
- `Set-Content -NoNewline` falhou em uma forma de chamada; prefira as ferramentas de edição de arquivo.
- Avisos de CRLF do git são esperados.
- `.codegraph/` é índice local de ferramenta — está no `.gitignore`.
- O scan completo leva ~11–15 s (parse do manifesto + globs); o app faz isso fora da thread da UI.

## CI

`.github/workflows/ci.yml`: em cada push na `main` e em PRs, roda `cargo test --workspace` e `cargo build -p ugs-app` no Ubuntu e no Windows. É a única verificação de build Linux disponível a partir da máquina Windows de desenvolvimento (o WSL dela não tem gcc; `cargo check --target x86_64-unknown-linux-gnu` falha no `ring`, que precisa de compilador C). Último run verde: commit `7b7aae6` (5 testes nos dois sistemas).

## Linux: build e pendências

- Dependências do app (Debian/Ubuntu): `libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libxdo-dev`.
- Pacotes: `cd app/src-tauri && npx @tauri-apps/cli@2 build` gera AppImage e .deb (`tauri.linux.conf.json`).
- Ainda não testado em máquina Linux real (Steam Deck é o alvo principal): detecção Steam/Proton/Heroic/Lutris, regras Linux/Flatpak do `extras.yaml`, bandeja no GNOME/KDE.

## Android

- **Acesso a arquivos:** em vez de SAF, o app pede a permissão nativa "Acesso a todos os arquivos" (`MANAGE_EXTERNAL_STORAGE`, Android 11+; `READ/WRITE_EXTERNAL_STORAGE` + `requestLegacyExternalStorage` até o Android 10). Com ela o `core` usa `std::fs` direto em `/storage/emulated/0`. Ok para APK distribuído fora da Play Store (a Play restringe essa permissão).
- `Android/data/<pacote>` de outros apps continua bloqueado no Android 11+. As regras para Dolphin, DuckStation, AetherSX2, Lemuroid e Vita3K já estão no `extras.yaml` e passam a funcionar quando houver acesso (próximo passo: Shizuku).
- Placeholder `<androidStorage>` (= `$EXTERNAL_STORAGE` ou `/storage/emulated/0`); `OS = "android"`; sem `<home>`/XDG.
- No Android o app não baixa o manifesto de PC: usa só `extras.yaml` (`manifest::with_extras(Manifest::new())`).
- Dados do app: `app.path().app_data_dir()`; backups em `/storage/emulated/0/UltimateGameSaver` (visível para Syncthing/Drive).
- Sem bandeja, monitor de processos nem seletor de pasta (`#[cfg(desktop)]`); a UI esconde pastas raiz, troca de pasta de backup e manifesto (`Settings.mobile`). O plugin de diálogo é dependência só de desktop.
- Sem `enableEdgeToEdge()` na `MainActivity` — a página não reserva espaço para a barra de status.
- Build (Windows): `$env:ANDROID_HOME="$env:LOCALAPPDATA\Android\Sdk"; $env:NDK_HOME="$env:ANDROID_HOME\ndk\<versão>"; cd app/src-tauri; npm install; npm run tauri -- android build --debug --apk --target aarch64` → `gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk` (~170 MB em debug).
- Teste no aparelho: `adb install -r <apk>`; liberar permissão sem tocar na tela: `adb shell appops set io.github.walterfr.ultimategamesaver MANAGE_EXTERNAL_STORAGE allow`.
- Aviso inofensivo no build: "Could not close incremental caches" do Kotlin (projeto em `X:` e cache do cargo em `C:`).

## Próximo passo

- Android: Shizuku para ler `Android/data`; APK release assinado anexado pelo `release.yml`; ícone próprio (`npm run tauri icon`).
- Fase 6: remapear IDs entre PCs/contas e sincronizar PC ↔ Android (o `mapping.yaml` já usa placeholders).

## Limitações conhecidas

- Status "Alterado" não percebe mudança só no registro.
- Backup automático só para jogos com `<base>` conhecido (loja ou pasta raiz); emuladores instalados em AppData sem pasta raiz não são monitorados.
- `<storeUserId>` vira `*` (pega todos os usuários Steam); remapeamento de IDs entre contas (Steam, Switch, 3DS, Xenia) é a Fase 6.
- Pastas de ROM vêm só das pastas raiz; frontends (LaunchBox, Playnite, ES-DE) ainda não são lidos.
- Xbox/Game Pass (`wgs`) não tem tratamento especial.
