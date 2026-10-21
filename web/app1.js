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

// ---------- badge (stand-in for the team logo) ----------
function badgeHtml(a, size) {
  const c = teamCol(a);
  return `<span class="badge" style="width:${size}px;height:${size}px;background:${c};color:${onColor(c)};font-size:${Math.max(8, size * (a.length > 2 ? .3 : .38))}px">${esc(a)}</span>`;
}
function drawBadge(ctx, a, x, y, r, alpha = 1) {
  const c = teamCol(a);
  ctx.save(); ctx.globalAlpha = alpha;
  ctx.fillStyle = c; ctx.beginPath(); ctx.arc(x, y, r, 0, Math.PI * 2); ctx.fill();
  ctx.fillStyle = onColor(c); ctx.font = `700 ${r * (a.length > 2 ? .62 : .8)}px system-ui,sans-serif`;
  ctx.textAlign = 'center'; ctx.textBaseline = 'middle'; ctx.fillText(a, x, y + r * .04);
  ctx.restore();
}

// ---------- router ----------
const backBtn = $('#back');
backBtn.onclick = () => history.back();
let stopViewer = null;
function route() {
  closeModal();
  if (stopViewer) { stopViewer(); stopViewer = null; }
  showErr(null);
  const parts = location.hash.replace(/^#\/?/, '').split('/').filter(Boolean);
  backBtn.style.display = parts.length ? 'inline-block' : 'none';
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
window.addEventListener('resize', () => { if (!parts0() && mapRedraw) mapRedraw(); });
const parts0 = () => location.hash.replace(/^#\/?/, '');
const go = h => { location.hash = h; };

// ---------- modal ----------
function closeModal() { $('#modal').style.display = 'none'; mapState = null; if (mapRedraw && !parts0()) mapRedraw(); }
$('#modal').addEventListener('click', e => { if (e.target.id === 'modal') closeModal(); });

// ---------- map ----------
let mapState = null, mapRedraw = null;
function albers(lon, lat) {
  const r = Math.PI / 180, p1 = 29.5 * r, p2 = 45.5 * r, p0 = 23 * r, l0 = -96 * r;
  const n = (Math.sin(p1) + Math.sin(p2)) / 2, c = Math.cos(p1) ** 2 + 2 * n * Math.sin(p1);
  const rho = p => Math.sqrt(Math.max(0, c - 2 * n * Math.sin(p))) / n;
  const th = n * (lon * r - l0), rr = rho(lat * r);
  return [rr * Math.sin(th), rho(p0) - rr * Math.cos(th)];
}
function spread(pts, minDist, iters) {
  const p = pts.map(a => a.slice());
  for (let it = 0; it < iters; it++) {
    let moved = false;
    for (let i = 0; i < p.length; i++) for (let j = i + 1; j < p.length; j++) {
      let dx = p[j][0] - p[i][0], dy = p[j][1] - p[i][1], d = Math.hypot(dx, dy);
      if (d >= minDist) continue;
      moved = true;
      let ux, uy;
      if (d < 1e-3) { const a = i * 7 + j * 13; ux = Math.cos(a); uy = Math.sin(a); } else { ux = dx / d; uy = dy / d; }
      const push = (minDist - d) / 2 + .01;
      p[i][0] -= ux * push; p[i][1] -= uy * push; p[j][0] += ux * push; p[j][1] += uy * push;
    }
    if (!moved) break;
  }
  return p;
}
function mapView() {
  app.innerHTML = '<canvas id="map"></canvas>';
  const cv = $('#map');
  const draw = () => {
    const W = window.innerWidth - 24, H = window.innerHeight - 44 - 24;
    const dpr = window.devicePixelRatio || 1;
    cv.style.height = H + 'px'; cv.width = W * dpr; cv.height = H * dpr;
    const ctx = cv.getContext('2d'); ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.fillStyle = '#13213A'; ctx.beginPath(); ctx.roundRect(0, 0, W, H, 8); ctx.fill();
    const proj = META.outline.map(([lo, la]) => albers(lo, la));
    const x0 = Math.min(...proj.map(p => p[0])), x1 = Math.max(...proj.map(p => p[0]));
    const y0 = Math.min(...proj.map(p => p[1])), y1 = Math.max(...proj.map(p => p[1]));
    const L = 16, T = 16, w = W - 32, h = H - 32;
    const sc = Math.min(w / (x1 - x0), h / (y1 - y0));
    const ox = L + (w - (x1 - x0) * sc) / 2, oy = T + (h - (y1 - y0) * sc) / 2;
    const scr = (lo, la) => { const [x, y] = albers(lo, la); return [ox + (x - x0) * sc, oy + (y1 - y) * sc]; };
    ctx.beginPath(); META.outline.forEach(([lo, la], i) => { const [x, y] = scr(lo, la); i ? ctx.lineTo(x, y) : ctx.moveTo(x, y); }); ctx.closePath();
    ctx.fillStyle = '#1B2C4B'; ctx.fill(); ctx.strokeStyle = '#2A3B5C'; ctx.lineWidth = 1.5; ctx.stroke();
    const logo = Math.max(26, Math.min(52, sc * .035), 40);
    const at = META.teams.map(t => scr(t.lon, t.lat));
    const sp = spread(at, logo * .95, 60).map(([x, y]) => [Math.max(logo / 2 + 2, Math.min(W - logo / 2 - 2, x)), Math.max(logo / 2 + 2, Math.min(H - logo / 2 - 2, y))]);
    mapHits = [];
    META.teams.forEach((t, i) => {
      const [sx, sy] = at[i], [x, y] = sp[i];
      if (Math.hypot(x - sx, y - sy) > 2) {
        ctx.strokeStyle = '#9AA8BD'; ctx.lineWidth = 1; ctx.beginPath(); ctx.moveTo(sx, sy); ctx.lineTo(x, y); ctx.stroke();
        ctx.fillStyle = '#9AA8BD'; ctx.beginPath(); ctx.arc(sx, sy, 2.5, 0, 7); ctx.fill();
      }
      const sel = mapState && mapState === t.state;
      if (sel) { ctx.strokeStyle = '#D50A0A'; ctx.lineWidth = 3; ctx.beginPath(); ctx.arc(x, y, logo * .62, 0, 7); ctx.stroke(); }
      drawBadge(ctx, t.a, x, y, logo / 2, mapState && !sel ? .45 : 1);
      mapHits.push({ t, x, y, sx, sy });
    });
    mapRedraw = draw; mapLimit = W * .09;
  };
  let mapHits = [], mapLimit = 0;
  cv.addEventListener('click', e => {
    const r = cv.getBoundingClientRect(), x = e.clientX - r.left, y = e.clientY - r.top;
    let best = null, bd = 1e9;
    mapHits.forEach(h => { const d = Math.hypot(h.x - x, h.y - y); if (d < logo0(h) && d < bd) { best = h; bd = d; } });
    if (!best) mapHits.forEach(h => { const d = Math.hypot(h.sx - x, h.sy - y); if (d < mapLimit && d < bd) { best = h; bd = d; } });
    if (best) openState(best.t.state);
  });
  const logo0 = () => 26;
  draw();
}
function openState(code) {
  mapState = code;
  const list = META.teams.filter(t => t.state === code);
  $('#mbox').innerHTML = `<h3>${esc(META.states[code])}<button class="x" id="mx">✕</button></h3>
    <div class="muted small" style="margin:2px 0 10px">${list.length} team${list.length === 1 ? '' : 's'}</div>` +
    list.map(t => `<div class="row" data-t="${t.a}" style="min-height:64px">${badgeHtml(t.a, 44)}<div class="grow"><div class="name">${esc(t.n)}</div><div class="muted small">${esc(t.city)}</div></div><span class="muted">▶</span></div>`).join('');
  $('#modal').style.display = 'flex';
  $('#mx').onclick = closeModal;
  document.querySelectorAll('#mbox .row').forEach(r => r.onclick = () => { const a = r.dataset.t; $('#modal').style.display = 'none'; go('team/' + a); });
  if (mapRedraw) mapRedraw();
}

// ---------- team ----------
async function teamView(abbr, tab) {
  app.innerHTML = '<p class="muted">Loading…</p>';
  const d = await loadTeam(abbr), t = TEAMS[abbr];
  let w = 0, l = 0, tie = 0, po = 0, pd = 0;
  d.games.forEach(g => { po += g.po; pd += g.pd; if (g.score) { const [a, b] = g.score; a > b ? w++ : a < b ? l++ : tie++; } });
  const rec = tie ? `${w}–${l}–${tie}` : `${w}–${l}`;
  let h = `<div style="display:flex;align-items:center;gap:12px;margin-top:6px">${badgeHtml(abbr, 64)}<div><div style="font-size:22px;font-weight:600">${esc(t.n)}</div><div class="muted small">${abbr} · ${esc(t.city)}</div></div></div>
    <div class="cards"><div class="stat"><b>${rec}</b><span>Record, weeks 1–8</span></div><div class="stat"><b>${thousands(po)}</b><span>Dropbacks on offense</span></div><div class="stat"><b>${thousands(pd)}</b><span>Dropbacks faced</span></div></div>
    <div class="tabs"><button class="tab ${tab === 'roster' ? 'on' : ''}" data-tab="roster">Roster</button><button class="tab ${tab === 'games' ? 'on' : ''}" data-tab="games">Season matches</button></div>`;
  if (tab === 'games') {
    h += d.games.map(g => {
      let when = g.when + (g.neutral ? ' · Neutral · ' + (g.stadium || 'neutral site') : '');
      const col = g.res ? (g.res[0] === 'W' ? '#3FB950' : g.res[0] === 'L' ? '#F85149' : '#8B949E') : '#9AA8BD';
      return `<div class="row" data-g="${g.id}">${badgeHtml(g.opp, 40)}<div class="grow"><div class="name">Week ${g.week} · ${esc(g.label)}</div><div class="muted small">${esc(when)}</div></div>
        <div style="text-align:right"><div style="color:${col};font-family:ui-monospace,monospace">${esc(g.res || '')}</div><div class="muted small">${g.po} off · ${g.pd} def</div></div></div>`;
    }).join('');
  } else {
    const r = d.roster.slice().sort((a, b) => a.order - b.order || b.snaps - a.snaps);
    let group = '';
    r.forEach(p => {
      if (p.group !== group) { group = p.group; h += `<h2>${esc(group)}</h2>`; }
      h += `<div class="row" data-p="${p.id}"><span class="num">${p.jersey != null ? '#' + p.jersey : '–'}</span><div class="grow"><div class="name">${esc(p.name)}</div>
        <div class="muted small"><span class="dot" style="background:${p.roleCol}"></span>${esc(p.pos)} · ${esc(p.roleL)}</div></div><span class="snaps">${thousands(p.snaps)}</span></div>`;
    });
  }
  app.innerHTML = h;
  app.querySelectorAll('.tab').forEach(b => b.onclick = () => history.replaceState(null, '', '#/team/' + abbr + '/' + b.dataset.tab) || route());
  app.querySelectorAll('[data-p]').forEach(r => r.onclick = () => go(`player/${r.dataset.p}/${abbr}`));
  app.querySelectorAll('[data-g]').forEach(r => r.onclick = () => go('game/' + r.dataset.g));
}

// ---------- player ----------
async function playerView(id, abbr) {
  app.innerHTML = '<p class="muted">Loading…</p>';
  if (!abbr) abbr = Object.keys(TEAMDATA).find(a => TEAMDATA[a].stats[id]);
  if (!abbr) throw new Error('Open a player from a team roster.');
  const d = await loadTeam(abbr), p = d.stats[id];
  if (!p) throw new Error('No stats for this player.');
  const bio = [p.height, p.weight ? p.weight + ' lb' : null, p.college, p.age != null ? 'age ' + p.age : null].filter(Boolean).join(' · ');
  let h = `<div style="display:flex;align-items:center;gap:12px;margin-top:6px">${badgeHtml(p.team || abbr, 64)}<div><div style="font-size:24px;font-weight:600">${esc(p.name)}</div>
    <div class="muted">${esc(p.pos)} · ${esc(p.posL)} · ${esc(p.roleL)} · <a href="#/team/${esc(p.team || abbr)}" style="color:#60A5FA">${esc((TEAMS[p.team || abbr] || {}).n || '')}</a></div><div class="muted small">${esc(bio)}</div></div></div>
    <div class="cards">${p.numbers.map(([l, v]) => `<div class="stat"><b>${esc(v)}</b><span>${esc(l)}</span></div>`).join('')}</div>`;
  p.breakdowns.forEach(b => {
    const total = b.bars.reduce((s, x) => s + x[1], 0) || 1, max = Math.max(1, ...b.bars.map(x => x[1]));
    h += `<div class="card"><b>${esc(b.t)}</b>` + b.bars.map(([l, v, c]) => `<div class="bar"><span class="l">${esc(l)}</span><span class="t"><i style="width:${(v / max * 100).toFixed(1)}%;background:${c}"></i></span><span class="v">${v % 1 ? v.toFixed(1) : v} · ${Math.round(v / total * 100)}%</span></div>`).join('') + '</div>';
  });
  if (p.trend.length) h += `<div class="card"><b>${esc(p.trendTitle)}</b><canvas id="trend" height="150"></canvas></div>`;
  h += '<h2>Games</h2>' + p.games.map(g => `<div class="row" data-g="${g.gid}">${badgeHtml(g.opp, 36)}<div class="grow"><div class="name">Week ${g.week} · ${g.at} ${esc(g.opp)}</div><div class="muted small">${esc(g.headline)}</div></div><span class="snaps">${g.snaps} snaps</span></div>`).join('');
  app.innerHTML = h;
  app.querySelectorAll('[data-g]').forEach(r => r.onclick = () => go('game/' + r.dataset.g));
  const cv = $('#trend');
  if (cv) {
    const W = cv.clientWidth, H = 150, dpr = window.devicePixelRatio || 1; cv.width = W * dpr; cv.height = H * dpr;
    const c = cv.getContext('2d'); c.setTransform(dpr, 0, 0, dpr, 0, 0);
    const xs = p.trend.map(t => t[0]), ys = p.trend.map(t => t[1]);
    const xmin = Math.min(...xs), xmax = Math.max(...xs, xmin + 1), ymax = Math.max(...ys, 1e-9), ymin = Math.min(0, ...ys);
    const px = x => 30 + (x - xmin) / (xmax - xmin) * (W - 50), py = y => H - 24 - (y - ymin) / (ymax - ymin || 1) * (H - 44);
    c.strokeStyle = '#2A3B5C'; c.beginPath(); c.moveTo(30, H - 24); c.lineTo(W - 10, H - 24); c.stroke();
    c.strokeStyle = '#60A5FA'; c.lineWidth = 2; c.beginPath(); p.trend.forEach(([x, y], i) => i ? c.lineTo(px(x), py(y)) : c.moveTo(px(x), py(y))); c.stroke();
    c.fillStyle = '#9AA8BD'; c.font = '11px system-ui'; c.textAlign = 'center';
    p.trend.forEach(([x, y]) => { c.fillStyle = '#60A5FA'; c.beginPath(); c.arc(px(x), py(y), 3.5, 0, 7); c.fill(); c.fillStyle = '#9AA8BD'; c.fillText('W' + x, px(x), H - 8); c.fillText(+y.toFixed(2), px(x), py(y) - 8); });
  }
}
