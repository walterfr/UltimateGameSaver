const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const $ = (s) => document.querySelector(s);

let rows = [];

function human(b) {
  const u = ['B', 'KB', 'MB', 'GB'];
  let i = 0;
  while (b >= 1024 && i < u.length - 1) { b /= 1024; i++; }
  return `${i ? b.toFixed(1) : b} ${u[i]}`;
}

function ago(s) {
  if (!s) return 'nunca';
  const d = Date.now() / 1000 - s;
  if (d < 60) return 'agora';
  if (d < 3600) return `há ${Math.floor(d / 60)} min`;
  if (d < 86400) return `há ${Math.floor(d / 3600)} h`;
  return new Date(s * 1000).toLocaleDateString('pt-BR');
}

function snapDate(file) {
  const m = file.match(/(\d{4})-(\d\d)-(\d\d)T(\d\d)-(\d\d)-(\d\d)Z/);
  if (!m) return file;
  return new Date(Date.UTC(m[1], m[2] - 1, m[3], m[4], m[5], m[6])).toLocaleString('pt-BR');
}

function status(r) {
  if (!r.on_pc) return ['only', 'Só no backup'];
  if (!r.last_backup) return ['never', 'Sem backup'];
  return r.changed ? ['changed', 'Alterado'] : ['ok', 'Em dia'];
}

function el(tag, props = {}, ...children) {
  const e = Object.assign(document.createElement(tag), props);
  e.append(...children);
  return e;
}

function toast(message, { error = false, action } = {}) {
  const t = el('div', { className: 'toast' + (error ? ' err' : '') }, el('span', { textContent: message }));
  if (action) t.append(el('button', { textContent: action.label, onclick: () => { t.remove(); action.run(); } }));
  $('#toasts').append(t);
  setTimeout(() => t.remove(), action ? 15000 : 8000);
}

/** Desativa o botão enquanto a ação roda; erros viram aviso. */
async function busy(btn, fn) {
  btn.disabled = true;
  try { await fn(); } catch (e) { toast(String(e), { error: true }); } finally { btn.disabled = false; }
}

async function refresh() {
  rows = await invoke('list');
  render();
}

async function doRestore(name, snapshot) {
  const r = await invoke('restore', { name, snapshot });
  toast('✔ ' + r.message, r.undo ? { action: { label: 'Desfazer', run: () => doRestore(name, r.undo).catch((e) => toast(String(e), { error: true })) } } : {});
  await refresh();
}

function rowEl(r) {
  const [cls, label] = status(r);
  const parts = [`${r.files} arquivo(s)`, human(r.size)];
  if (r.registry) parts.push(`${r.registry} chave(s) de registro`);
  if (r.watched) parts.push('backup automático ao fechar');

  const snaps = el('ul', { className: 'snaps', hidden: true });
  const backupBtn = el('button', { textContent: 'Backup', disabled: !r.on_pc });
  const restoreBtn = el('button', { textContent: 'Restaurar', disabled: !r.last_backup, title: 'Restaura o último backup (o estado atual é guardado antes)' });
  const moreBtn = el('button', { textContent: '▾', disabled: !r.last_backup, title: 'Escolher outro backup', ariaLabel: 'Escolher outro backup' });

  backupBtn.onclick = () => busy(backupBtn, async () => {
    for (const m of await invoke('backup', { names: [r.name] })) toast(m);
    await refresh();
  });
  restoreBtn.onclick = () => busy(restoreBtn, () => doRestore(r.name, null));
  moreBtn.onclick = () => busy(moreBtn, async () => {
    if (!snaps.hidden) { snaps.hidden = true; return; }
    const list = await invoke('snapshots', { name: r.name });
    snaps.replaceChildren(...list.map((s) => {
      const b = el('button', { textContent: 'Restaurar este' });
      b.onclick = () => busy(b, () => doRestore(r.name, s.file));
      const label = `${snapDate(s.file)} · ${human(s.size)}${s.pre_restore ? ' · estado antes de restaurar' : ''}`;
      return el('li', {}, el('span', { textContent: label }), b);
    }));
    snaps.hidden = false;
  });

  return el('li', { className: 'row' },
    el('div', { className: 'name', textContent: r.name }),
    el('div', { className: 'meta', textContent: r.on_pc ? parts.join(' · ') : 'Nenhum save no PC — dá para restaurar' }),
    el('span', { className: `badge ${cls}`, textContent: label }),
    el('div', { className: 'when', textContent: ago(r.last_backup), title: 'Último backup' }),
    el('div', { className: 'actions' }, backupBtn, restoreBtn, moreBtn),
    snaps,
  );
}

function render() {
  const q = $('#search').value.trim().toLowerCase();
  const shown = rows.filter((r) => r.name.toLowerCase().includes(q));
  $('#list').replaceChildren(...shown.map(rowEl));
  const pending = rows.filter((r) => r.on_pc && (r.changed || !r.last_backup)).length;
  $('#summary').textContent = `${rows.length} jogo(s) · ${pending} precisando de backup`;
}

async function scan() {
  $('#summary').textContent = 'Escaneando…';
  rows = await invoke('scan');
  render();
}

// ---- configurações ----
function showSettings(s) {
  document.querySelectorAll('.desktop-only').forEach((e) => { e.hidden = s.mobile; });
  $('#android-hint').hidden = !s.mobile;
  $('#backup-dir').textContent = s.backup_dir;
  $('#roots').replaceChildren(...s.roots.map((dir) => {
    const b = el('button', { textContent: 'Remover' });
    b.onclick = () => busy(b, async () => showSettings(await invoke('remove_root', { dir })));
    return el('li', {}, el('span', { textContent: dir }), b);
  }));
  if (!s.roots.length) $('#roots').append(el('li', { className: 'hint', textContent: 'Nenhuma pasta raiz.' }));
}

$('#open-settings').onclick = async () => { showSettings(await invoke('settings')); $('#settings').showModal(); };
$('#add-root').onclick = (e) => busy(e.target, async () => showSettings(await invoke('add_root')));
$('#change-backup-dir').onclick = (e) => busy(e.target, async () => { showSettings(await invoke('choose_backup_dir')); await refresh(); });
$('#update-manifest').onclick = (e) => busy(e.target, async () => toast(await invoke('update_manifest')));
$('#settings').addEventListener('close', () => scan().catch((e) => toast(String(e), { error: true })));

$('#search').oninput = render;
$('#rescan').onclick = (e) => busy(e.target, scan);
$('#backup-all').onclick = (e) => busy(e.target, async () => {
  const msgs = await invoke('backup_all');
  const made = msgs.filter((m) => m.startsWith('✔')).length;
  const failed = msgs.filter((m) => m.startsWith('✘'));
  toast(`Backup de tudo: ${made} novo(s), ${msgs.length - made - failed.length} sem mudança`);
  failed.forEach((m) => toast(m, { error: true }));
  await refresh();
});

listen('auto-backup', (e) => { e.payload.forEach((m) => toast(m, { error: m.startsWith('✘') })); refresh(); });

scan().catch((e) => { $('#summary').textContent = 'Erro: ' + e; });
