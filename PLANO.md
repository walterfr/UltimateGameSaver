# Ultimate Game Saver — Pesquisa e Plano

Organizador de saves de jogos de PC e emuladores, com backup automático e restauração em um clique.

> Caminhos abaixo são os **padrões no Windows**. Quase todo emulador tem **modo portátil** (saves dentro da pasta do executável) e permite mudar a pasta no config — por isso o app deve **ler o arquivo de configuração do emulador** antes de assumir o padrão.

---

## Parte 1 — Pesquisa

### 1.1 Tipos de dado de save (o que existe)

| Tipo | Exemplos | Portável entre versões? | Observação |
|---|---|---|---|
| **SRAM / bateria** (save "real" do cartucho) | `.srm .sav .eep .sra .fla .mpk .dsv` | Sim | Arquivo bruto. Prioridade nº 1. |
| **Memory card em imagem** | `.mcd .mcr .ps2 .raw .gci .bkr .bcr .bin (VMU)` | Sim | Um arquivo guarda vários jogos (exceto modos "por jogo"). |
| **Sistema de arquivos do console** | pastas PSP/PS3/Vita/3DS/Wii/Wii U/Switch/360 | Sim | Pasta por título, com ID do usuário no caminho. |
| **Imagem de disco** | `xbox_hdd.qcow2` (xemu) | Sim | Save fica dentro da imagem; backup do arquivo inteiro. |
| **Save state** | `.state .ss1 .p2s .sav(DuckStation) .mss .cst .ppst .SAVESTAT` | **Não** — preso à versão do emulador/core | Fazer backup, mas marcar como "frágil". |
| **Jogo de PC — arquivo** | `.sav .dat .json .es3 .save` etc. | Sim | Local varia por jogo. |
| **Jogo de PC — registro** | `HKCU\Software\<Empresa>\<Jogo>` (Unity PlayerPrefs) | Sim | Exportar/importar chaves (`.reg`). |
| **Contêiner protegido** | Xbox/Game Pass `wgs\` | Parcial | Ligado à conta Xbox; restauração arriscada. |

### 1.2 Plataformas de PC

| Plataforma | Onde detectar jogos instalados | Onde ficam saves |
|---|---|---|
| **Steam** | `HKCU\Software\Valve\Steam\SteamPath`; `steamapps\libraryfolders.vdf`; `appmanifest_<AppID>.acf` | Steam Cloud: `<Steam>\userdata\<SteamID3>\<AppID>\remote\` (+ muitos jogos salvam fora, ver 1.3). Usuários em `config\loginusers.vdf`. |
| **Epic Games** | `C:\ProgramData\Epic\EpicGamesLauncher\Data\Manifests\*.item` (JSON) | Geralmente Unreal: `%LOCALAPPDATA%\<Projeto>\Saved\SaveGames\*.sav` |
| **GOG Galaxy** | `HKLM\SOFTWARE\WOW6432Node\GOG.com\Games\<id>`; `C:\ProgramData\GOG.com\Galaxy\storage\galaxy-2.0.db` (SQLite) | Varia por jogo (ver 1.3) |
| **Ubisoft Connect** | `HKLM\SOFTWARE\WOW6432Node\Ubisoft\Launcher\Installs\<id>` | `C:\Program Files (x86)\Ubisoft\Ubisoft Game Launcher\savegames\<UserID>\<GameID>\*.save` |
| **EA app** | `C:\ProgramData\EA Desktop\` + registro `HKLM\SOFTWARE\WOW6432Node\<Publisher>` | Normalmente `Documents\<Jogo>` ou `Documents\Electronic Arts\` |
| **Battle.net** | `C:\ProgramData\Battle.net\Agent\product.db` | Maioria só nuvem; exceções (ex.: `Saved Games\Diablo II Resurrected`) |
| **Rockstar Launcher** | registro `HKLM\SOFTWARE\WOW6432Node\Rockstar Games\` | `Documents\Rockstar Games\<Jogo>\Profiles\<ID>\` |
| **Xbox app / Game Pass** | `Get-AppxPackage` | `%LOCALAPPDATA%\Packages\<PackageFamilyName>\SystemAppData\wgs\<XUID>_<id>\` (`containers.index` + blobs) |
| **Amazon Games** | `%LOCALAPPDATA%\Amazon Games\Data\Games\Sql\GameInstallInfo.sqlite` | Varia por jogo |
| **itch.io / Heroic / Playnite / Lutris** | DBs/JSON próprios do launcher | Varia; Heroic/Lutris (Wine): prefixo `drive_c\users\<user>\...` |
| **Proton (Steam no Linux/Steam Deck)** | `steamapps/compatdata/<AppID>/` | `.../pfx/drive_c/users/steamuser/<mesmos caminhos do Windows>` |

### 1.3 Locais comuns de saves de jogos de PC (Windows)

| Local | Quem usa |
|---|---|
| `%USERPROFILE%\Documents\My Games\<Jogo>` | Bethesda, Firaxis, muitos AAA antigos |
| `%USERPROFILE%\Documents\<Empresa ou Jogo>` | Muito comum |
| `%USERPROFILE%\Saved Games\<Jogo>` (FOLDERID_SavedGames) | Jogos que seguem a diretriz da Microsoft |
| `%APPDATA%\<Jogo>` (Roaming) | Muito comum |
| `%LOCALAPPDATA%\<Projeto>\Saved\SaveGames\` | **Unreal Engine** |
| `%USERPROFILE%\AppData\LocalLow\<Empresa>\<Produto>\` | **Unity** (arquivos) |
| `HKCU\Software\<Empresa>\<Produto>` | **Unity PlayerPrefs** (registro) |
| `%APPDATA%\Godot\app_userdata\<Projeto>\` | **Godot** |
| `%LOCALAPPDATA%\<Jogo>\` | **GameMaker** |
| `%APPDATA%\RenPy\<jogo-id>\` + `<jogo>\game\saves\` | **Ren'Py** |
| `<pasta do jogo>\Save\` / `www\save\` | **RPG Maker** (dentro da instalação) |
| `%LOCALAPPDATA%\VirtualStore\Program Files (x86)\<Jogo>\` | Jogos antigos que escreviam em Program Files (redirecionamento UAC) |
| `<pasta do jogo>\save*` | Indies, jogos antigos, DRM-free |

**Conclusão:** não há regra única para PC. A fonte de verdade é o **PCGamingWiki**, já compilado em formato de máquina pelo projeto **Ludusavi Manifest** (`github.com/mtkennerly/ludusavi-manifest`, `data/manifest.yaml`, dezenas de milhares de jogos, com placeholders `<base>`, `<home>`, `<storeUserId>`, `<winAppData>`, `<winLocalAppData>`, `<winDocuments>`, `<regHkcu>` etc.). **Reusar esse manifesto em vez de catalogar à mão.**

### 1.4 Emuladores

Legenda: **SRAM** = save de bateria/memory card (seguro), **ST** = save state (frágil). `<E>` = pasta do emulador (modo portátil).

#### Multi-sistema
| Emulador | Onde configurar | SRAM | ST |
|---|---|---|---|
| **RetroArch** | `retroarch.cfg`: `savefile_directory`, `savestate_directory`, `sort_savefiles_enable`, `sort_savestates_enable` | `<E>\saves\[<core>\]<rom>.srm` (alguns cores: `.sav`, memcards PSX `.srm`/`.mcr`) | `<E>\states\<rom>.state[N]`, `.state.auto` |
| **ares** | settings do ares | `<pasta configurada>` ou ao lado da ROM | idem |
| **Mednafen** | `mednafen.cfg` | `<E>\sav\*.sav .mcr .bkr .bcr` | `<E>\mcs\*.mc0-9` |
| **MAME** | `mame.ini` (`nvram_directory`, `state_directory`) | `<E>\nvram\<jogo>\`, `<E>\hiscore\` (plugin), `<E>\cfg\` | `<E>\sta\<jogo>\` |
| **FinalBurn Neo** (standalone) | — | `<E>\config\games\`, `<E>\nvram\` | `<E>\savestates\*.fs` |

#### Nintendo
| Sistema | Emulador | SRAM / save | ST |
|---|---|---|---|
| NES | **Mesen** | `Documents\Mesen2\Saves\*.sav` (ou `<E>\Saves`) | `SaveStates\*.mss` |
| NES | **FCEUX** | `<E>\sav\*.sav` | `<E>\fcs\*.fc0-9` |
| SNES | **Snes9x** | `<E>\Saves\*.srm` | `<E>\Saves\*.000-.009` |
| SNES | **bsnes** | ao lado da ROM ou pasta configurada, `.srm` | `.bsz` |
| N64 | **Project64** | `<E>\Save\*.eep .sra .fla .mpk` | `<E>\Save\*.pj[0-9]` |
| N64 | **simple64 / Mupen64Plus / RMG** | `%APPDATA%\Mupen64Plus\save\` ou `<E>\Save\` | `.st0-9` |
| GB/GBC/GBA | **mGBA** | `.sav` **ao lado da ROM** (padrão) | `.ss1-.ss9` |
| GB/GBA | **VisualBoyAdvance-M** | `.sav` ao lado da ROM ou pasta configurada no `vbam.ini` | `.sgm` |
| NDS | **melonDS** | `.sav` ao lado da ROM (configurável) | `.ml1-.ml8` |
| NDS | **DeSmuME** | `<E>\Battery\*.dsv` | `<E>\States\*.ds0-9` |
| 3DS | **Citra / Lime3DS / Azahar** | `%APPDATA%\<Azahar|Citra|Lime3DS>\sdmc\Nintendo 3DS\<ID0>\<ID1>\title\<high>\<low>\data\` (+ `extdata\`) | `states\*.cst` |
| GameCube | **Dolphin** | `Documents\Dolphin Emulator\` **ou** `%APPDATA%\Dolphin Emulator\` (builds novas; também `HKCU\Software\Dolphin Emulator\UserConfigPath`; portátil se `portable.txt`): `GC\<USA|EUR|JAP>\Card A\*.gci` (GCI folder) ou `GC\MemoryCardA.<REG>.raw` | `StateSaves\<ID>.s01-.s10` |
| Wii | **Dolphin** | `Wii\title\00010000\<ID do jogo em hex>\data\` | idem |
| Wii U | **Cemu** | `<mlc01>\usr\save\00050000\<TitleID low>\user\80000001\` (mlc em `<E>\mlc01` portátil ou `%APPDATA%\Cemu\mlc01`; ver `settings.xml` → `mlc_path`) | — |
| Switch | **Ryujinx** (+ forks como Ryubing) | `%APPDATA%\Ryujinx\bis\user\save\<0000000000000xxx>\0\` — índice em `bis\system\save\8000000000000000\0\imkvdb.arc` mapeia pasta→TitleID | — |
| Switch | **yuzu** e forks (**Suyu, Sudachi, Citron, Eden**) | `%APPDATA%\<nome>\nand\user\save\0000000000000000\<UserID 32 hex>\<TitleID 16 hex>\` (ver `qt-config.ini` → `nand_directory`) | — |

#### Sony
| Sistema | Emulador | SRAM / save | ST |
|---|---|---|---|
| PS1 | **DuckStation** | `Documents\DuckStation\memcards\*.mcd` (padrão por jogo: `<serial>_1.mcd`; ou `shared_card_1.mcd`); portátil se `portable.txt` | `savestates\*.sav` |
| PS1 | **ePSXe** | `<E>\memcards\epsxe000.mcr` | `<E>\sstates\` |
| PS1 | **PCSX-Redux** | `<E>\memcard1.mcd` | — |
| PS2 | **PCSX2** | `Documents\PCSX2\memcards\Mcd001.ps2` (arquivo) ou pasta (folder memcard); ver `inis\PCSX2.ini` → `[Folders]` | `sstates\*.p2s` |
| PS3 | **RPCS3** | `<E>\dev_hdd0\home\00000001\savedata\<SERIAL…>\` (`PARAM.SFO`, `ICON0.PNG`) + troféus `…\00000001\trophy\` | `<E>\savestates\` |
| PSP | **PPSSPP** | `Documents\PPSSPP\PSP\SAVEDATA\<ULUS…>\` ou `<E>\memstick\PSP\SAVEDATA\` (ver `ppsspp.ini`) | `PSP\PPSSPP_STATE\*.ppst` |
| Vita | **Vita3K** | `%APPDATA%\Vita3K\Vita3K\ux0\user\00\savedata\<TITLEID>\` (ou pasta portátil) | — |
| PS4 | **shadPS4** | `<E>\user\savedata\1\<CUSA…>\` ou `%APPDATA%\shadPS4\savedata\` | — |

#### Microsoft
| Sistema | Emulador | Save |
|---|---|---|
| Xbox | **xemu** | Dentro de `xbox_hdd.qcow2` (caminho no `xemu.toml`) — backup da imagem inteira |
| Xbox 360 | **Xenia / Xenia Canary** | `Documents\Xenia\content\<XUID do perfil>\<TitleID>\00000001\` (versões antigas: `content\0000000000000000\…`); portátil: `<E>\content\` |

#### Sega e outros
| Sistema | Emulador | Save |
|---|---|---|
| Mega Drive/CD | Genesis Plus GX (RetroArch), Kega Fusion, BlastEm | `.srm`; Sega CD BRAM `.brm` |
| Saturn | Beetle Saturn (RA), Mednafen, Yaba Sanshiro | `.bkr` (interno), `.bcr` (cartucho) |
| Dreamcast | **Flycast** | `<E>\data\vmu_save_A1.bin` (ou VMU por jogo); ST `.state` |
| Dreamcast | **Redream** | `<E>\vmu*.bin` |
| PC Engine, NGP, Lynx, WonderSwan | Mednafen / RA | `.sav` |
| Atari 2600/7800 | Stella / A7800 | só states |
| Jaguar | BigPEmu | `%APPDATA%\BigPEmu\` |
| DOS | DOSBox / DOSBox-X / DOSBox Staging | Dentro da pasta montada do jogo |
| Point-and-click | **ScummVM** | `%APPDATA%\ScummVM\Saved games\*.s##` |

#### Frontends (fonte de detecção, não de saves)
LaunchBox, Playnite, ES-DE, Pegasus, EmuDeck: guardam **lista de ROMs e emuladores configurados** — úteis para descobrir onde estão os emuladores e as pastas de ROM (necessário para mGBA/melonDS/bsnes que salvam ao lado da ROM).

### 1.5 Linux

| Item | Caminho |
|---|---|
| Steam | `~/.local/share/Steam` (ou `~/.steam/steam`); Flatpak: `~/.var/app/com.valvesoftware.Steam/.local/share/Steam` |
| Jogos Windows via Proton | `<Steam>/steamapps/compatdata/<AppID>/pfx/drive_c/users/steamuser/` + caminhos Windows de 1.3 |
| Heroic / Lutris / Bottles | Prefixos Wine de cada jogo (`~/Games/Heroic/Prefixes/<jogo>`, `~/Games/<jogo>`, `~/.local/share/bottles/bottles/<nome>`) |
| Jogos nativos | XDG: `~/.local/share/<jogo>`, `~/.config/<jogo>`, Unity `~/.config/unity3d/<Empresa>/<Produto>` |
| Emuladores nativos | Seguem XDG: `~/.config/retroarch/saves`, `~/.local/share/dolphin-emu`, `~/.config/PCSX2`, `~/.local/share/duckstation`, `~/.config/ppsspp`, `~/.config/rpcs3`, `~/.local/share/Cemu`, `~/.local/share/<yuzu-fork>`, `~/.config/Ryujinx`, `~/.local/share/azahar-emu` |
| Emuladores Flatpak | Mesmo caminho, mas sob `~/.var/app/<app-id>/` (ex.: `org.DolphinEmu.dolphin-emu`, `net.pcsx2.PCSX2`, `org.libretro.RetroArch`) |
| AppImage | Igual ao nativo; modo portátil via pasta `<AppImage>.home` / `.config` ao lado |
| EmuDeck / RetroDECK | EmuDeck: `~/Emulation/saves/<emulador>/` (symlinks para as pastas reais); RetroDECK: `~/retrodeck/saves`, `~/retrodeck/states` |

O manifesto do Ludusavi já traz caminhos Linux (`when: os: linux`) e regras para Proton.

### 1.6 Android

**Restrição central:** desde o Android 11 (scoped storage), um app não lê `/Android/data/<pacote>/` de outro app. Desde o Android 13, nem o seletor de pastas (SAF) dá acesso a ela. Caminhos de acesso:

| Caso | Como acessar |
|---|---|
| Save em armazenamento compartilhado (`/storage/emulated/0/...`) | SAF: usuário escolhe a pasta uma vez (`ACTION_OPEN_DOCUMENT_TREE`), permissão persistente |
| Save em `/Android/data/<pacote>/files` | **Shizuku** (ADB sem root) ou **root**. Sem isso: pedir ao usuário para mudar a pasta de dados do emulador para o armazenamento compartilhado (a maioria permite) |
| Save em `/data/data/<pacote>` (privado) | Só root |

| Emulador Android | Save (padrão — **verificar em aparelho**) |
|---|---|
| RetroArch | `/storage/emulated/0/RetroArch/saves`, `states` (configurável) |
| Lemuroid | `Android/data/com.swordfish.lemuroid/files/saves` (pasta de ROMs escolhida via SAF) |
| Dolphin | `Android/data/org.dolphinemu.dolphinemu/files/` (mesma estrutura GC/Wii do PC) |
| PPSSPP | `/storage/emulated/0/PSP/SAVEDATA` ou pasta escolhida |
| DuckStation | `Android/data/com.github.stenzek.duckstation/files/memcards` |
| AetherSX2 / NetherSX2 | `Android/data/xyz.aethersx2.android/files/memcards` |
| Azahar / Lime3DS / Citra | Pasta escolhida pelo usuário na 1ª execução, mesma estrutura `sdmc/Nintendo 3DS/...` |
| Eden / Citron / Sudachi (Switch) | `Android/data/<pacote>/files/nand/user/save/...` (mesma estrutura do PC) |
| Vita3K | `Android/data/org.vita3k.emulator/files/vita3k/ux0/user/00/savedata` |
| DraStic | `/storage/emulated/0/DraStic/backup/*.dsv` |
| melonDS Android | `.sav` ao lado da ROM ou pasta escolhida |
| My Boy! / Pizza Boy | `.sav` ao lado da ROM |
| Mupen64Plus FZ | `Android/data/org.mupen64plusae.v3.fzurita/files/` |
| Winlator / GameHub | Prefixo Wine em armazenamento privado; só root (ou backup manual pelo próprio app) |

**Vantagem:** a estrutura de saves é a mesma no PC e no Android para Dolphin, PPSSPP, DuckStation, PCSX2/AetherSX2, Azahar e os forks do yuzu. Isso permite **sincronizar o mesmo jogo entre PC e celular**, que é o recurso de maior valor deste app.

### 1.7 Decomps, recomps e ports nativos

Jogos de console recompilados para PC. Não aparecem em loja nenhuma, quase nunca têm instalador e muitos são **portáteis** (save ao lado do `.exe`). Entradas marcadas **(verificar)** precisam ser confirmadas instalando o port.

**Padrões por família** (cobrem dezenas de ports com uma regra só):

| Família | Ports | Save (Windows) | Save (Linux) |
|---|---|---|---|
| **N64Recomp** (`<Nome>Recompiled`) | Zelda64Recomp (Majora's Mask) e os demais recomps da família | `%LOCALAPPDATA%\<Nome>Recompiled\saves\` | `~/.config/<Nome>Recompiled/saves/` (verificar) |
| **XenonRecomp** (Xbox 360) | UnleashedRecomp (Sonic Unleashed), MarathonRecomp (Sonic '06) | `%APPDATA%\<Nome>Recomp\save\` | `~/.config/<Nome>Recomp/save/` (verificar) |
| **Harbour Masters / libultraship** | Ship of Harkinian (OoT), 2 Ship 2 Harkinian (MM), Starship (SF64), SpaghettiKart (MK64) | `<pasta do exe>\Save\*.sav` (JSON nas versões novas) + `<nome>.json` (config) | AppImage: pasta de dados do app (verificar) |
| **snesrev** | zelda3, smw, sm | `<pasta do exe>\saves\` | idem |
| **sm64 PC** | sm64ex, Render96ex, sm64coopdx, sm64-port | `%APPDATA%\sm64ex\sm64_save_file.bin` (coopdx: `%APPDATA%\sm64coopdx\`); sm64-port: ao lado do exe | `~/.local/share/sm64ex/` |
| **Retro Engine (RSDK)** | Sonic 1/2 (v4), Sonic CD (v3), Sonic Mania (v5) | `<pasta do jogo>\` (`SData.bin`) / `SaveData\` (verificar) | idem |

**Ports individuais:**

| Port | Save (Windows) | Save (Linux) |
|---|---|---|
| **OpenGOAL** (Jak 1/2/3) | `%APPDATA%\OpenGOAL\<jak1|jak2|jak3>\saves\` | `~/.local/share/OpenGOAL/<jogo>/saves/` |
| **Perfect Dark** (port) | `%APPDATA%\perfectdark\` ou ao lado do exe (verificar) | `~/.local/share/perfectdark/` (verificar) |
| **DevilutionX** (Diablo) | `%APPDATA%\diasurgical\devilution\*.sv` | `~/.local/share/diasurgical/devilution/` |
| **Sonic 3 A.I.R.** | `%APPDATA%\Sonic3AIR\` | `~/.local/share/Sonic3AIR/` (verificar) |
| **wipeout-rewrite** | `save.dat` ao lado do exe | idem |

**Reimplementações de engine** (precisam do jogo original; mesma lógica de detecção):

| Engine | Save (Windows) |
|---|---|
| OpenMW (Morrowind) | `Documents\My Games\OpenMW\saves\` |
| OpenRCT2 | `Documents\OpenRCT2\save\` |
| OpenTTD | `Documents\OpenTTD\save\` |
| OpenXcom | `Documents\OpenXcom\` |
| GZDoom / UZDoom | `Documents\My Games\GZDoom\` (versões antigas: ao lado do exe) |
| fheroes2 | `%APPDATA%\fheroes2\files\save\` (verificar) |
| Julius / Augustus (Caesar III) | pasta do jogo original |
| CorsixTH | `%APPDATA%\CorsixTH\Saves\` (verificar) |
| ioquake3 / dhewm3 / Yamagi Quake II | `%APPDATA%\<engine>\` / `Documents\My Games\<engine>\` |

**Como detectar** (não há registro nem loja):
1. Procurar **executáveis conhecidos** (`soh.exe`, `2ship.exe`, `Zelda64Recompiled.exe`, `UnleashedRecomp.exe`, `sm64ex.exe`, `gk.exe`, `devilutionx.exe`…) nas pastas que o usuário indicar e em locais comuns (`Downloads`, `Desktop`, `C:\Games`, `~/Games`, `~/Applications`).
2. Checar pastas de dados fixas (`%LOCALAPPDATA%\*Recompiled`, `%APPDATA%\OpenGOAL` etc.) — achar o save mesmo sem achar o exe.
3. Ler frontends (Playnite, LaunchBox, Steam "jogos não-Steam" em `userdata\<id>\config\shortcuts.vdf`), onde usuários costumam cadastrar esses ports.

**Cuidados:**
- Save ao lado do exe some quando o usuário **apaga a pasta para atualizar o port** — principal motivo para fazer backup automático desses jogos.
- Formatos variam: alguns usam o formato do cartucho, outros JSON próprio. Converter save de emulador ↔ port só onde o formato for comprovadamente igual (fase futura, não no MVP).
- Ports mudam de pasta entre versões — guardar a versão no `mapping.yaml` e manter regras por versão quando necessário.

---

## Parte 2 — Plano

### 2.1 Decisão principal: reusar, não reinventar

- **Jogos de PC:** usar o **manifesto do Ludusavi** (YAML, atualizado continuamente a partir do PCGamingWiki). Baixar `manifest.yaml` na primeira execução e atualizar com ETag/If-None-Match.
- **Opção mais preguiçosa e válida:** usar o próprio **Ludusavi CLI** como motor (`ludusavi backup --api`, `ludusavi restore --api`, saída JSON) e o app ser só a interface + módulo de emuladores. Decidir isso antes de codar (ver 2.9).
- **Emuladores:** o manifesto do Ludusavi não cobre emuladores. Criar `emulators.yaml` **no mesmo formato** (mesmos placeholders + novos: `<emuDir>`, `<romDir>`, `<emuConfig:chave>`), para um único motor de backup tratar tudo.
- **Decomps/recomps/ports:** `ports.yaml`, mesmo formato, com placeholder `<exeDir>` (pasta onde o executável foi achado) e lista de nomes de executável para detecção. Antes de escrever uma entrada, checar se o manifesto do Ludusavi já a tem (alguns ports têm página no PCGamingWiki).

### 2.2 Modelo de dados (único para PC e emulador)

```yaml
# entrada de emulators.yaml
Dolphin:
  detect:
    registry: HKCU\Software\Dolphin Emulator\UserConfigPath
    paths: ["<winDocuments>/Dolphin Emulator", "<winAppData>/Dolphin Emulator", "<emuDir>"]
    portableMarker: portable.txt
  files:
    "<root>/GC/**":            { tags: [save] }
    "<root>/Wii/title/**":     { tags: [save] }
    "<root>/StateSaves/**":    { tags: [state] }   # frágil
  perGame:                      # como quebrar em jogos individuais
    "<root>/GC/*/Card A/*.gci": { id: fromFilename }
    "<root>/Wii/title/00010000/*/data": { id: hexToAscii(dir) }
```

Granularidade: emuladores que guardam **um save por jogo** (Dolphin GCI, PCSX2 folder memcard, RPCS3, PPSSPP, Switch, 3DS, Xenia, SRAM de RetroArch) aparecem **por jogo**. Memory cards compartilhados (`Mcd001.ps2`, `shared_card_1.mcd`) aparecem como **um item "memory card"**.

### 2.3 Detecção (scanner)

1. **Launchers de PC:** ler Steam (`libraryfolders.vdf`, `appmanifest_*.acf`, `loginusers.vdf`), Epic (`*.item`), GOG (registro), Ubisoft, EA, Xbox (`Get-AppxPackage`), Heroic/Playnite se existirem.
2. **Cruzar com o manifesto** pelo ID da loja (Steam AppID, GOG ID) ou nome; expandir placeholders; **só listar entradas cujo caminho exista** no disco.
3. **Emuladores:** procurar em (a) registro de desinstalação, (b) caminhos padrão, (c) pastas que o usuário adicionar, (d) frontends. Para cada um, **ler o config** (`retroarch.cfg`, `PCSX2.ini`, `settings.ini`, `ppsspp.ini`, `settings.xml`, `qt-config.ini`, `Config.json`, `xemu.toml`) para achar a pasta real.
4. **Varredura opcional "saves órfãos":** procurar em `Saved Games`, `My Games`, `AppData\LocalLow`, `Saved\SaveGames` pastas não reconhecidas e oferecer adicionar como jogo customizado.

### 2.4 Formato do backup

```
<PastaBackup>/
  <Plataforma>/<Jogo>/
    2026-11-19T21-04-00.zip        # snapshot completo (zip, Deflate)
    2026-11-20T18-30-12.zip
    mapping.yaml                   # índice: snapshots, origem de cada arquivo
```

`mapping.yaml` por snapshot guarda:
- caminho original **com placeholders** (`<winAppData>/Cemu/mlc01/...`), não o caminho absoluto — permite restaurar em outro PC/usuário;
- `sha256`, tamanho, mtime de cada arquivo; chaves de registro exportadas como `.reg` dentro do zip;
- IDs dependentes de usuário: SteamID3, UserID do Switch, XUID do Xenia, ID0/ID1 do 3DS, usuário do RPCS3;
- versão do emulador (para avisar sobre save states);
- tags `save` / `state`.

Regras:
- **Pular backup se nada mudou** (comparar hash/mtime com o último snapshot) — evita lixo.
- **Retenção:** manter últimos N (padrão 10) + 1 por semana do último mês.
- Escrita **atômica**: gerar `.zip.tmp`, validar, renomear.
- Nuvem: **não implementar**. Usuário aponta a pasta de backup para OneDrive/Google Drive/Dropbox/Syncthing. (rclone só se pedirem.)
- Dedup por conteúdo (estilo restic) só se o tamanho virar problema — RPCS3/xemu/Cemu podem ser grandes.

### 2.5 Gatilhos de backup

| Gatilho | Como |
|---|---|
| Manual "Backup agora" (um jogo / tudo) | Botão |
| **Ao fechar o jogo/emulador** (principal) | Monitorar processos (WMI `Win32_ProcessStopTrace` ou polling a cada 5 s) do executável conhecido; backup ~10 s depois de fechar |
| Agendado | Timer interno ou Agendador de Tarefas do Windows |
| Ao vivo (opcional) | `FileSystemWatcher` com debounce de 30 s — cuidado: jogos escrevem saves parcialmente |

### 2.6 Restauração em um clique

Fluxo do botão **"Restaurar"** (padrão = último snapshot; menu ▾ para escolher outro):

1. **Checar processo:** jogo/emulador aberto → bloquear com aviso ("Feche o Dolphin antes").
2. **Backup de segurança automático** do estado atual (`pre-restore-<data>.zip`) — torna o restore desfazível com outro clique.
3. **Resolver placeholders** para a máquina atual. Se ID de usuário mudou (outro SteamID, outra conta Switch, outro XUID, outro ID0 do 3DS) → remapear para o ID atual; se houver vários, perguntar uma vez e lembrar.
4. **Extrair para pasta temporária, verificar sha256**, depois mover para o destino (substitui só os arquivos do snapshot; não apaga arquivos extras, a menos que "restauração exata" esteja marcada).
5. Importar `.reg` se houver.
6. Mostrar resultado: "✔ 14 arquivos restaurados · desfazer".

Casos especiais que exigem regra própria:

| Caso | Regra |
|---|---|
| **Steam Cloud** | Steam pode sobrescrever o save restaurado. Avisar; sugerir restaurar com Steam fechado e escolher "local" no conflito. |
| **Xbox / Game Pass (`wgs`)** | Ligado à conta e ao índice `containers.index`. Restaurar só na mesma conta; marcar como "experimental". |
| **Ryujinx** | Pasta de save numérica é mapeada por `imkvdb.arc`. Exigir que o jogo tenha sido aberto uma vez no destino; achar a pasta pelo TitleID e copiar só o conteúdo de `0\`. |
| **yuzu e forks** | Trocar `<UserID>` pelo do perfil atual. |
| **3DS** | Trocar `<ID0>/<ID1>` pelos existentes no destino. |
| **Xenia** | Trocar XUID do perfil. |
| **Memory card compartilhado (PS1/PS2)** | Restaura o arquivo inteiro — avisar que afeta todos os jogos daquele card. |
| **Save states** | Avisar se a versão do emulador mudou. |
| **Saves ao lado da ROM** | Se a ROM mudou de lugar, procurar ROM pelo nome/hash e restaurar ao lado dela. |

### 2.7 Interface

- **Lista/grade de jogos** agrupada por plataforma (Steam, Epic, …, Dolphin, PCSX2, …) com: último backup, tamanho, status (✔ em dia / ⚠ mudou desde o último backup).
- Dois botões por jogo: **Backup** e **Restaurar**. Botões globais: **Backup tudo**, **Escanear**.
- Ícone na bandeja: roda em segundo plano para o gatilho "ao fechar o jogo"; notificação discreta.
- **CLI** com os mesmos comandos (`ugs backup --all`, `ugs restore "Zelda TP"`) — útil para atalho no desktop/Steam/Playnite (restaurar em um clique fora do app).
- Tela "Adicionar jogo/pasta personalizada".

### 2.8 Fases

| Fase | Entrega | Verificação |
|---|---|---|
| **1. Motor** | Ler manifesto Ludusavi, expandir placeholders, detectar Steam/Epic/GOG, backup zip + mapping, restore com backup de segurança | Teste: backup → apagar save → restore → hash igual |
| **2. Emuladores top** | `emulators.yaml` com RetroArch, Dolphin, PCSX2, DuckStation, PPSSPP, RPCS3, Cemu, Ryujinx/forks yuzu, Azahar, Xenia, mGBA, melonDS | Pastas de exemplo (fixtures) por emulador |
| **3. Um clique + automação** | UI, bandeja, gatilho ao fechar processo, retenção, CLI | Abrir/fechar emulador real gera snapshot |
| **4. Remapeamento** | IDs de usuário (Steam, Switch, 3DS, Xenia), ROM movida, outro PC | Restaurar backup de outra conta Windows |
| **5. Resto** | Emuladores restantes, Xbox `wgs`, Ubisoft/EA/Rockstar, varredura de órfãos | — |

### 2.9 Decisões tomadas

- **Motor próprio** que reusa o manifesto do Ludusavi.
- **Plataformas:** Windows, Linux e Android.
- **Linguagem:** Rust.

### 2.10 Arquitetura Rust

```
ultimate-game-saver/            (workspace Cargo)
  core/        crate lib: manifesto, placeholders, scanner, backup/restore, zip, mapping.yaml
  cli/         binário `ugs` (Windows/Linux) — atalho de um clique, scripts
  app/         Tauri 2: UI única (desktop + Android)
    src-tauri/ comandos Tauri chamando `core`
    android/   plugin Kotlin: SAF + Shizuku
```

- **Por que Tauri 2:** uma só UI (HTML/TS) para Windows, Linux e Android; backend Rust; plugins nativos em Kotlin onde o Android exige. Alternativas consideradas: egui/iced/Slint (UI 100% Rust, mas Android imaturo ou acesso SAF manual) e Compose no Android + UniFFI (UI duplicada).
- **I/O no Android:** SAF não é `std::fs` — arquivos vêm como `content://` via `ContentResolver`. Para não espalhar isso pelo motor: `core` trabalha com um trait de armazenamento com duas implementações reais — `std::fs` (desktop, Shizuku/root) e SAF (ponte para o plugin Kotlin). Estratégia mais simples para começar no Android: o plugin Kotlin **copia** a pasta SAF para o cache do app, `core` faz backup com `std::fs`, e o restore faz o caminho inverso.
- **Crates:** `serde` + `serde_yaml` (manifesto), `zip`, `sha2`, `walkdir`, `globset`, `directories` (pastas XDG/Windows), `winreg` (só Windows), `sysinfo` (detecção de processo aberto/fechado), `notify` (opcional, monitorar arquivos), `reqwest` (baixar manifesto).
- **Sincronização PC ↔ Android:** pasta de backup em serviço de nuvem já sincronizado (Drive/Syncthing). Como `mapping.yaml` usa placeholders, um snapshot feito no PC restaura no celular e vice-versa, nos emuladores com a mesma estrutura (ver 1.6).

### 2.11 Fases revisadas

| Fase | Entrega |
|---|---|
| 1 ✔ | `core` + `cli` no Windows: manifesto, Steam/Epic/GOG, backup/restore, teste de ida e volta |
| 2 | Linux: XDG, Flatpak, Proton, Heroic/Lutris |
| 3 ✔ | `core/data/extras.yaml` (emuladores + ports, um arquivo só), pastas raiz (`ugs root add`), detecção por executável, pasta custom lida do config, saves ao lado da ROM |
| 4 | App Tauri desktop: lista, um clique, bandeja, backup ao fechar o jogo |
| 5 | Android: plugin SAF, emuladores Android, depois Shizuku |
| 6 | Remapeamento de IDs, sincronização PC ↔ Android, restante dos emuladores |
