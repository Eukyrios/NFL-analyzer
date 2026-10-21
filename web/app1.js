'use strict';
if (!CanvasRenderingContext2D.prototype.roundRect) CanvasRenderingContext2D.prototype.roundRect = function (x, y, w, h) { this.rect(x, y, w, h); };
const $ = (s, e = document) => e.querySelector(s);
const app = $('#app');
const TEAMS = {}; META.teams.forEach(t => TEAMS[t.a] = t);
const GAMES = (window.GAMES = window.GAMES || {});
const META_GAME = {}; META.games.forEach(g => META_GAME[g.id] = g);
const esc = s => String(s ?? '').replace(/[&<>"]/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
const thousands = n => String(n).replace(/\B(?=(\d{3})+(?!\d))/g, ',');
const hexRgb = h => [1, 3, 5].map(i => parseInt(h.slice(i, i + 2), 16));
const lum = h => { const f = v => { v /= 255; return v <= .03928 ? v / 12.92 : Math.pow((v + .055) / 1.055, 2.4); }; const [r, g, b] = hexRgb(h); return .2126 * f(r) + .7152 * f(g) + .0722 * f(b); };
const isLight = h => lum(h) > .35;
const onColor = h => isLight(h) ? '#111111' : '#ffffff';
const teamCol = a => (TEAMS[a] ? TEAMS[a].c : '#6E7681');
const mix = (h, k) => { const [r, g, b] = hexRgb(h); const m = v => Math.round(v * k); return `rgb(${m(r)},${m(g)},${m(b)})`; };
const setTitle = t => { $('#title').textContent = t; };
const toast = m => { const t = $('#toast'); t.textContent = m; t.style.opacity = 1; clearTimeout(toast.h); toast.h = setTimeout(() => t.style.opacity = 0, 1700); };

// ---------- logos ----------
const LOGO_IMG = {};
Object.keys(window.LOGOS || {}).forEach(a => { const i = new Image(); i.src = LOGOS[a]; LOGO_IMG[a] = i; });
function logoHtml(a, size) {
  if (LOGOS[a]) return `<span class="logo" style="width:${size}px;height:${size}px"><img src="${LOGOS[a]}" alt="${esc(a)}"></span>`;
  const c = teamCol(a);
  return `<span class="logo fb" style="width:${size}px;height:${size}px;background:${c};color:${onColor(c)};font-size:${Math.max(8, size * .3)}px">${esc(a)}</span>`;
}
// Draws a logo disc on a canvas; falls back to a colored badge when there is no image (e.g. WAS).
function drawLogo(ctx, a, x, y, r, alpha = 1) {
  ctx.save(); ctx.globalAlpha = alpha;
  const img = LOGO_IMG[a];
  if (img && img.complete && img.naturalWidth) {
    ctx.fillStyle = '#EEF2F8'; ctx.beginPath(); ctx.arc(x, y, r, 0, Math.PI * 2); ctx.fill();
    ctx.drawImage(img, x - r * .84, y - r * .84, r * 1.68, r * 1.68);
  } else {
    const c = teamCol(a);
    ctx.fillStyle = c; ctx.beginPath(); ctx.arc(x, y, r, 0, Math.PI * 2); ctx.fill();
    ctx.fillStyle = onColor(c); ctx.font = `800 ${r * (a.length > 2 ? .62 : .8)}px system-ui,sans-serif`;
    ctx.textAlign = 'center'; ctx.textBaseline = 'middle'; ctx.fillText(a, x, y + r * .04);
  }
  ctx.restore();
}

// ---------- script loading ----------
const loaded = {};
function loadScript(src) {
  return loaded[src] ||= new Promise((res, rej) => {
    const s = document.createElement('script');
    s.src = src; s.onload = () => res(); s.onerror = () => { delete loaded[src]; rej(new Error('Could not load ' + src)); };
    document.head.appendChild(s);
  });
}
const TEAMDATA = (window.TEAMDATA = window.TEAMDATA || {});
const loadTeam = a => TEAMDATA[a] ? Promise.resolve(TEAMDATA[a]) : loadScript(`data/team_${a}.js`).then(() => TEAMDATA[a]);
const loadGame = id => GAMES[id] ? Promise.resolve(GAMES[id]) : loadScript(`data/game_${id}.js`).then(() => GAMES[id]);
function showErr(e) { $('#err').textContent = e ? String(e.message || e) : ''; }

// ---------- router ----------
const backBtn = $('#back');
backBtn.onclick = () => history.back();
let stopViewer = null, stopMap = null;
function route() {
  closeSheet(true);
  if (stopViewer) { stopViewer(); stopViewer = null; }
  if (stopMap) { stopMap(); stopMap = null; }
  showErr(null);
  const parts = location.hash.replace(/^#\/?/, '').split('/').filter(Boolean);
  backBtn.style.display = parts.length ? 'flex' : 'none';
  window.scrollTo(0, 0);
  const done = p => p && p.catch && p.catch(e => { showErr(e); app.innerHTML = `<p class="muted">Something went wrong: ${esc(e.message)}</p>`; });
  try {
    if (parts[0] === 'team') done(teamView(parts[1], parts[2] || 'roster'));
    else if (parts[0] === 'player') done(playerView(+parts[1], parts[2]));
    else if (parts[0] === 'game') done(gameView(+parts[1], parts[2] ? +parts[2] : null));
    else mapView();
  } catch (e) { showErr(e); }
}
window.addEventListener('hashchange', route);
const go = h => { location.hash = h; };

// ---------- bottom sheet ----------
let sheetOnClose = null;
function closeSheet(silent) { $('#modal').style.display = 'none'; if (sheetOnClose && !silent) sheetOnClose(); sheetOnClose = null; }
$('#modal').addEventListener('click', e => { if (e.target.id === 'modal') closeSheet(); });

// ---------- map ----------
function spread(pts, minDist, iters) {
  const p = pts.map(a => a.slice());
  for (let it = 0; it < iters; it++) {
    let moved = false;
    for (let i = 0; i < p.length; i++) for (let j = i + 1; j < p.length; j++) {
      const dx = p[j][0] - p[i][0], dy = p[j][1] - p[i][1], d = Math.hypot(dx, dy);
      if (d >= minDist) continue;
      moved = true;
      let ux, uy; if (d < 1e-3) { const a = i * 7 + j * 13; ux = Math.cos(a); uy = Math.sin(a); } else { ux = dx / d; uy = dy / d; }
      const push = (minDist - d) / 2 + .01;
      p[i][0] -= ux * push; p[i][1] -= uy * push; p[j][0] += ux * push; p[j][1] += uy * push;
    }
    if (!moved) break;
  }
  return p;
}
function mapView() {
  setTitle('NFL Play Viewer');
  const hasTeam = {}; META.teams.forEach(t => (hasTeam[t.state] ||= []).push(t));
  app.innerHTML = `<div id="mapwrap"><svg id="msvg"><g id="mg">${USMAP.states.map(s => `<path class="st${hasTeam[s.code] ? ' has' : ''}" data-state="${s.code}" d="${s.d}"/>`).join('')}</g><g id="conn" stroke="rgba(255,255,255,.35)" stroke-width="1"></g></svg>
    <div id="mlogos"></div><div id="hint">Tap a state to see its teams · pinch to zoom</div>
    <div id="zoom"><button id="zin">+</button><button id="zout">−</button><button id="zfit" style="font-size:14px">⤢</button></div></div>
    <h2>States with teams</h2><div id="schips" style="display:flex;flex-wrap:wrap;gap:8px">${Object.keys(hasTeam).sort((a, b) => META.states[a].localeCompare(META.states[b])).map(c => `<button class="schip" data-s="${c}">${hasTeam[c].map(t => logoHtml(t.a, 24)).join('')}<span>${esc(META.states[c])}</span></button>`).join('')}</div>`;
  const wrap = $('#mapwrap'), mg = $('#mg'), conn = $('#conn'), ml = $('#mlogos');
  let W = 0, H = 0, k = 1, tx = 0, ty = 0, k0 = 1, sel = null, cur = [];
  const LOGO = () => (W < 600 ? 34 : 52);
  ml.innerHTML = META.teams.map(t => `<div class="ml" data-t="${t.a}" data-state="${t.state}">${logoHtml(t.a, LOGO())}</div>`).join('');
  const els = [...ml.children];
  function size() {
    W = wrap.clientWidth; H = W < 600 ? Math.max(300, W * .98) : Math.max(360, Math.min(window.innerHeight - 54 - 28, W * .64)); wrap.style.height = H + 'px';
    els.forEach(e => { const s = LOGO(); e.firstChild.style.width = e.firstChild.style.height = s + 'px'; });
  }
  const fitView = () => { k0 = k = Math.min(W / USMAP.w, H / USMAP.h) * (W < 600 ? 1.0 : .96); tx = (W - USMAP.w * k) / 2; ty = (H - USMAP.h * k) / 2; };
  const clamp = () => { const mw = USMAP.w * k, mh = USMAP.h * k; tx = mw < W ? (W - mw) / 2 : Math.min(40, Math.max(W - mw - 40, tx)); ty = mh < H ? (H - mh) / 2 : Math.min(40, Math.max(H - mh - 40, ty)); };
  function draw() {
    clamp(); mg.setAttribute('transform', `translate(${tx},${ty}) scale(${k})`);
    const L = LOGO(), at = META.teams.map(t => { const xy = USMAP.xy[t.a]; return xy ? [xy[0] * k + tx, xy[1] * k + ty] : [-99, -99]; });
    const sp = spread(at, L * 1.02, 50).map(([x, y]) => [Math.max(L / 2, Math.min(W - L / 2, x)), Math.max(L / 2 + 24, Math.min(H - L / 2, y))]);
    let c = ''; cur = sp;
    META.teams.forEach((t, i) => {
      const e = els[i]; e.style.transform = `translate(${sp[i][0] - L / 2}px,${sp[i][1] - L / 2}px)`; e.classList.toggle('dim', !!sel && sel !== t.state);
      if (Math.hypot(sp[i][0] - at[i][0], sp[i][1] - at[i][1]) > 4) c += `<line x1="${at[i][0]}" y1="${at[i][1]}" x2="${sp[i][0]}" y2="${sp[i][1]}"/><circle cx="${at[i][0]}" cy="${at[i][1]}" r="2.5" fill="#fff" stroke="none"/>`;
    });
    conn.innerHTML = c;
    mg.querySelectorAll('.st').forEach(p => p.classList.toggle('sel', p.dataset.state === sel));
  }
  function zoomAt(px, py, f) { const nk = Math.max(k0 * .9, Math.min(k0 * 6, k * f)); tx = px - (px - tx) * (nk / k); ty = py - (py - ty) * (nk / k); k = nk; draw(); }
  size(); fitView(); draw();
  // pointer interaction: tap, drag, pinch
  const ptrs = new Map(); let down = null, pinch = 0;
  wrap.addEventListener('pointerdown', e => { ptrs.set(e.pointerId, [e.clientX, e.clientY]); down = { x: e.clientX, y: e.clientY, t: Date.now(), target: e.target, moved: false, tx, ty }; if (ptrs.size === 2) { const [a, b] = [...ptrs.values()]; pinch = Math.hypot(a[0] - b[0], a[1] - b[1]); down.moved = true; } });
  wrap.addEventListener('pointermove', e => {
    if (!ptrs.has(e.pointerId)) return; ptrs.set(e.pointerId, [e.clientX, e.clientY]);
    const r = wrap.getBoundingClientRect();
    if (ptrs.size === 2) { const [a, b] = [...ptrs.values()], d = Math.hypot(a[0] - b[0], a[1] - b[1]); if (pinch) zoomAt((a[0] + b[0]) / 2 - r.left, (a[1] + b[1]) / 2 - r.top, d / pinch); pinch = d; }
    else if (down && (Math.hypot(e.clientX - down.x, e.clientY - down.y) > 7 || down.moved)) { down.moved = true; tx = down.tx + e.clientX - down.x; ty = down.ty + e.clientY - down.y; draw(); }
  });
  const up = e => {
    const was = ptrs.has(e.pointerId); ptrs.delete(e.pointerId); if (ptrs.size < 2) pinch = 0;
    if (was && down && !down.moved && Date.now() - down.t < 600 && e.type === 'pointerup') tap(down.target);
    if (ptrs.size === 0) down = null;
  };
  wrap.addEventListener('pointerup', up); wrap.addEventListener('pointercancel', up);
  wrap.addEventListener('wheel', e => { e.preventDefault(); const r = wrap.getBoundingClientRect(); zoomAt(e.clientX - r.left, e.clientY - r.top, e.deltaY < 0 ? 1.18 : 1 / 1.18); }, { passive: false });
  $('#zin').onclick = e => { e.stopPropagation(); zoomAt(W / 2, H / 2, 1.5); }; $('#zout').onclick = e => { e.stopPropagation(); zoomAt(W / 2, H / 2, 1 / 1.5); }; $('#zfit').onclick = e => { e.stopPropagation(); fitView(); draw(); };
  ['zin', 'zout', 'zfit'].forEach(id => $('#' + id).addEventListener('pointerdown', e => e.stopPropagation()));
  // logos sit above the svg; use geometry to find a tapped logo
  function tap(target) {
    let code = null;
    const r = wrap.getBoundingClientRect(), px = down.x - r.left, py = down.y - r.top, L = LOGO();
    let best = null, bd = 1e9; cur.forEach((p, i) => { const d = Math.hypot(p[0] - px, p[1] - py); if (d < L / 2 + 6 && d < bd) { best = META.teams[i]; bd = d; } });
    if (best) code = best.state; else { const s = target.closest && target.closest('[data-state]'); if (s) code = s.dataset.state; }
    if (!code) return;
    if (!hasTeam[code]) { const st = USMAP.states.find(s => s.code === code); toast(`${st ? st.name : code} has no NFL team`); return; }
    openState(code);
  }
  function openState(code) {
    sel = code; draw();
    const list = hasTeam[code];
    $('#mbox').innerHTML = `<div class="grab"></div><div style="display:flex;justify-content:space-between;align-items:center;margin:0 2px 2px"><div><div style="font-size:22px;font-weight:700">${esc(META.states[code])}</div><div class="muted small">${list.length} team${list.length === 1 ? '' : 's'}</div></div>
      <button id="mx" style="width:36px;height:36px;border-radius:50%;border:1px solid var(--border);background:var(--raised)">✕</button></div><div class="list" style="margin-top:12px">` +
      list.map(t => `<div class="row" data-t="${t.a}" style="min-height:72px;border-left:5px solid ${t.c}">${logoHtml(t.a, 48)}<div class="grow"><div class="name" style="font-size:17px">${esc(t.n)}</div><div class="sub">${esc(t.city)}</div></div><span class="chev">›</span></div>`).join('') + '</div>';
    $('#modal').style.display = 'flex'; sheetOnClose = () => { sel = null; draw(); };
    $('#mx').onclick = () => closeSheet();
    document.querySelectorAll('#mbox .row').forEach(r => r.onclick = () => { const a = r.dataset.t; closeSheet(true); go('team/' + a); });
  }
  document.querySelectorAll('.schip').forEach(b => b.onclick = () => openState(b.dataset.s));
  const onResize = () => { size(); fitView(); draw(); };
  window.addEventListener('resize', onResize); stopMap = () => window.removeEventListener('resize', onResize);
}

// ---------- team ----------
function heroStyle(c) { return `background:linear-gradient(135deg,${c} 0%,${mix(c, .45)} 100%);color:${onColor(c)}`; }
async function teamView(abbr, tab) {
  app.innerHTML = '<p class="muted">Loading…</p>';
  const d = await loadTeam(abbr), t = TEAMS[abbr]; setTitle(t.n);
  let w = 0, l = 0, tie = 0, po = 0, pd = 0;
  d.games.forEach(g => { po += g.po; pd += g.pd; if (g.score) { const [a, b] = g.score; a > b ? w++ : a < b ? l++ : tie++; } });
  const rec = tie ? `${w}–${l}–${tie}` : `${w}–${l}`;
  let h = `<div class="hero" style="${heroStyle(t.c)}">${logoHtml(abbr, 72)}<div><div class="t1">${esc(t.n)}</div><div class="t2">${esc(t.city)}</div></div><div class="big">${abbr}</div></div>
    <div class="tiles"><div class="tile"><b>${rec}</b><span>Record (wk 1–8)</span></div><div class="tile"><b>${thousands(po)}</b><span>Dropbacks on offense</span></div><div class="tile"><b>${thousands(pd)}</b><span>Dropbacks faced</span></div></div>
    <div class="seg"><button data-tab="roster" class="${tab === 'roster' ? 'on' : ''}">Roster</button><button data-tab="games" class="${tab === 'games' ? 'on' : ''}">Season matches</button></div>`;
  if (tab === 'games') {
    h += '<div class="list" style="margin-top:10px">' + d.games.map(g => {
      const when = g.when + (g.neutral ? ' · Neutral · ' + (g.stadium || 'neutral site') : '');
      const k = g.res ? g.res[0] : '', bg = k === 'W' ? 'rgba(63,185,80,.18)' : k === 'L' ? 'rgba(248,81,73,.18)' : 'rgba(143,160,186,.18)', col = k === 'W' ? '#56d364' : k === 'L' ? '#ff7b72' : '#8FA0BA';
      return `<div class="row" data-g="${g.id}">${logoHtml(g.opp, 44)}<div class="grow"><div class="name">${esc(g.label)}</div><div class="sub">Week ${g.week} · ${esc(when)}</div></div>
        <div style="text-align:right">${g.res ? `<div class="wl" style="background:${bg};color:${col}">${esc(g.res)}</div>` : ''}<div class="sub" style="margin-top:3px">${g.po} off · ${g.pd} def</div></div></div>`;
    }).join('') + '</div>';
  } else {
    const r = d.roster.slice().sort((a, b) => a.order - b.order || b.snaps - a.snaps);
    let group = '', open = false;
    r.forEach(p => {
      if (p.group !== group) { if (open) h += '</div>'; group = p.group; h += `<h2>${esc(group)}</h2><div class="list">`; open = true; }
      h += `<div class="row" data-p="${p.id}"><span class="jersey" style="background:${t.c};color:${onColor(t.c)}">${p.jersey != null ? p.jersey : '–'}</span><div class="grow"><div class="name">${esc(p.name)}</div>
        <div class="sub"><span class="dot" style="background:${p.roleCol}"></span> ${esc(p.posL)} · ${esc(p.roleL)}</div></div><div class="snaps">${thousands(p.snaps)}<small>snaps</small></div></div>`;
    });
    if (open) h += '</div>';
  }
  app.innerHTML = h;
  app.querySelectorAll('.seg button').forEach(b => b.onclick = () => { history.replaceState(null, '', '#/team/' + abbr + '/' + b.dataset.tab); route(); });
  app.querySelectorAll('[data-p]').forEach(r => r.onclick = () => go(`player/${r.dataset.p}/${abbr}`));
  app.querySelectorAll('[data-g]').forEach(r => r.onclick = () => go('game/' + r.dataset.g));
}

// ---------- player ----------
async function playerView(id, abbr) {
  app.innerHTML = '<p class="muted">Loading…</p>';
  if (!abbr) abbr = Object.keys(TEAMDATA).find(a => TEAMDATA[a].stats[id]);
  if (!abbr) throw new Error('Open a player from a team roster.');
  const d = await loadTeam(abbr), p = d.stats[id], ros = d.roster.find(r => r.id === id) || {};
  if (!p) throw new Error('No stats for this player.');
  const tm = p.team || abbr, t = TEAMS[tm] || TEAMS[abbr]; setTitle(p.name);
  const bio = [p.height, p.weight ? p.weight + ' lb' : null, p.college, p.age != null ? 'age ' + p.age : null].filter(Boolean).join(' · ');
  let h = `<div class="hero" style="${heroStyle(t.c)}">${logoHtml(tm, 64)}<div style="min-width:0"><div class="t1">${esc(p.name)}</div><div class="t2">${esc(p.posL)} · ${esc(p.roleL)}</div><div class="t2 small" style="opacity:.75">${esc(bio)}</div></div>${ros.jersey != null ? `<div class="big">${ros.jersey}</div>` : ''}</div>
    <div class="tiles">${p.numbers.map(([l, v]) => `<div class="tile"><b>${esc(v)}</b><span>${esc(l)}</span></div>`).join('')}</div>`;
  p.breakdowns.forEach(b => {
    const total = b.bars.reduce((s, x) => s + x[1], 0) || 1, max = Math.max(1, ...b.bars.map(x => x[1]));
    h += `<div class="card"><b>${esc(b.t)}</b>` + b.bars.map(([l, v, c]) => `<div class="bar"><span class="l">${esc(l)}</span><span class="t"><i style="width:${(v / max * 100).toFixed(1)}%;background:${c}"></i></span><span class="v">${v % 1 ? v.toFixed(1) : v} · ${Math.round(v / total * 100)}%</span></div>`).join('') + '</div>';
  });
  if (p.trend.length) h += `<div class="card"><b>${esc(p.trendTitle)}</b><canvas id="trend" height="160"></canvas></div>`;
  h += '<h2>Game log</h2><div class="list">' + p.games.map(g => `<div class="row" data-g="${g.gid}">${logoHtml(g.opp, 40)}<div class="grow"><div class="name">Week ${g.week} · ${g.at} ${esc(g.opp)}</div><div class="sub">${esc(g.headline)}</div></div><div class="snaps">${g.snaps}<small>snaps</small></div></div>`).join('') + '</div>';
  app.innerHTML = h;
  app.querySelectorAll('[data-g]').forEach(r => r.onclick = () => go('game/' + r.dataset.g));
  const cv = $('#trend');
  if (cv) {
    const W = cv.clientWidth, H = 160, dpr = window.devicePixelRatio || 1; cv.width = W * dpr; cv.height = H * dpr; cv.style.height = H + 'px';
    const c = cv.getContext('2d'); c.setTransform(dpr, 0, 0, dpr, 0, 0);
    const xs = p.trend.map(t => t[0]), ys = p.trend.map(t => t[1]);
    const xmin = Math.min(...xs), xmax = Math.max(...xs, xmin + 1), ymax = Math.max(...ys, 1e-9), ymin = Math.min(0, ...ys);
    const px = x => 24 + (x - xmin) / (xmax - xmin) * (W - 48), py = y => H - 28 - (y - ymin) / (ymax - ymin || 1) * (H - 56);
    c.strokeStyle = '#1F3150'; c.beginPath(); c.moveTo(14, H - 28); c.lineTo(W - 10, H - 28); c.stroke();
    const grad = c.createLinearGradient(0, 20, 0, H - 28); grad.addColorStop(0, 'rgba(79,140,255,.35)'); grad.addColorStop(1, 'rgba(79,140,255,0)');
    c.beginPath(); p.trend.forEach(([x, y], i) => i ? c.lineTo(px(x), py(y)) : c.moveTo(px(x), py(y))); c.lineTo(px(xs[xs.length - 1]), H - 28); c.lineTo(px(xs[0]), H - 28); c.fillStyle = grad; c.fill();
    c.strokeStyle = '#4F8CFF'; c.lineWidth = 2.5; c.beginPath(); p.trend.forEach(([x, y], i) => i ? c.lineTo(px(x), py(y)) : c.moveTo(px(x), py(y))); c.stroke();
    c.font = '11px system-ui'; c.textAlign = 'center';
    p.trend.forEach(([x, y]) => { c.fillStyle = '#4F8CFF'; c.beginPath(); c.arc(px(x), py(y), 4, 0, 7); c.fill(); c.fillStyle = '#8FA0BA'; c.fillText('W' + x, px(x), H - 10); c.fillStyle = '#dfe6f1'; c.fillText(+y.toFixed(2), px(x), py(y) - 10); });
  }
}
