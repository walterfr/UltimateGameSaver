# AGENTS.md — Ultimate Game Saver

Guia para agentes de código (Antigravity, Claude Code, Codex, Cursor etc.) e pessoas que forem continuar este projeto.
Leia junto com [PLANO.md](PLANO.md), que tem a pesquisa completa de onde ficam os saves e o roadmap.

## O que é

Organizador de saves de jogos de PC e emuladores: acha os saves, faz backup versionado em zip e restaura com um clique.
Alvo: Windows (pronto), Linux e Android (planejados). Repo: https://github.com/walterfr/UltimateGameSaver

## Estado atual

| Fase | Status |
|---|---|
| 1. Motor + CLI no Windows (manifesto, Steam/Epic/GOG, backup/restore) | ✔ |
| 2. Linux | **próxima** |
| 3. Emuladores e ports (`core/data/extras.yaml`) | ✔ |
| 4. App desktop Tauri 2 + instalador NSIS | ✔ — Release v0.1.0 publicada |
| 5. Android | pendente |
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
app/ui/                 frontend HTML/CSS/JS puro, sem npm nem bundler
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
- Versão: alterar em `app/src-tauri/tauri.conf.json` e nos três `Cargo.toml`.
- Release: gerar o instalador, copiar para um nome sem espaços (`UltimateGameSaver_<versão>_x64-setup.exe`) e `gh release create v<versão> <arquivo> --notes-file <notas>`. O instalador **não é assinado** (SmartScreen avisa).

## Pegadinhas do ambiente (Windows)

- Shell padrão é PowerShell. `git commit -F -` com here-string não funciona — escreva a mensagem num arquivo e use `-F arquivo`.
- `Set-Content -NoNewline` falhou em uma forma de chamada; prefira as ferramentas de edição de arquivo.
- Avisos de CRLF do git são esperados.
- `.codegraph/` é índice local de ferramenta — está no `.gitignore`.
- O scan completo leva ~11–15 s (parse do manifesto + globs); o app faz isso fora da thread da UI.

## Próximo passo: Fase 2 (Linux)

O código já compila pensando em Linux em vários pontos (`cfg(windows)` no registro e GOG; `Env` com `<xdgData>`/`<xdgConfig>`; Steam em `~/.local/share/Steam`). Falta:
1. Steam: também `~/.steam/steam` e Flatpak `~/.var/app/com.valvesoftware.Steam/.local/share/Steam`.
2. Proton: jogos Windows rodando no Linux salvam em `steamapps/compatdata/<AppID>/pfx/drive_c/users/steamuser/...` — mapear os placeholders Windows para dentro do prefixo (o Ludusavi faz isso).
3. Heroic (`~/.config/heroic/`, prefixos em `~/Games/Heroic/Prefixes`) e Lutris (`~/.local/share/lutris/pga.db`).
4. Pastas raiz: executáveis sem extensão e AppImage já estão em `extras.yaml`; testar.
5. App Tauri no Linux: dependências `webkit2gtk-4.1`, `libayatana-appindicator3` (bandeja); empacotar AppImage/deb (`bundle.targets`).
6. Testar as regras Linux/Flatpak do `extras.yaml` em máquina real (Steam Deck é o alvo principal).

Depois: Fase 5 (Android) — ver PLANO.md §1.6 e §2.10 (Tauri 2 mobile + plugin Kotlin para SAF/Shizuku).

## Limitações conhecidas

- Status "Alterado" não percebe mudança só no registro.
- Backup automático só para jogos com `<base>` conhecido (loja ou pasta raiz); emuladores instalados em AppData sem pasta raiz não são monitorados.
- `<storeUserId>` vira `*` (pega todos os usuários Steam); remapeamento de IDs entre contas (Steam, Switch, 3DS, Xenia) é a Fase 6.
- Pastas de ROM vêm só das pastas raiz; frontends (LaunchBox, Playnite, ES-DE) ainda não são lidos.
- Xbox/Game Pass (`wgs`) não tem tratamento especial.
