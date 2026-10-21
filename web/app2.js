// ---------- game viewer ----------
const LEN = 120, WID = 53.3, FPS = 10, HASHES = [0.33, 23.58, 29.75, WID - 0.33];
const EVENT = { line_set: 'Set', man_in_motion: 'Motion', ball_snap: 'Snap', play_action: 'PA', pass_forward: 'Throw', qb_sack: 'Sack', qb_strip_sack: 'Sack', run: 'Run', pass_arrived: 'Arrived', pass_outcome_caught: 'Caught', pass_outcome_incomplete: 'Inc', autoevent_passinterrupted: 'Interrupted', fumble: 'Fumble' };
const ROLE_COL = { 'Pass': '#FACC15', 'Pass Block': '#60A5FA', 'Pass Route': '#2DD4BF', 'Pass Rush': '#F87171', 'Coverage': '#C084FC' };
const ROLE_NAME = { 'Pass': 'Passer', 'Pass Block': 'Pass block', 'Pass Route': 'Route', 'Pass Rush': 'Pass rush', 'Coverage': 'Coverage' };
const RES_NAMES = [['C', 'Complete', '#3FB950'], ['I', 'Incomplete', '#8B949E'], ['S', 'Sack', '#F85149'], ['IN', 'Interception', '#F0883E'], ['R', 'Scramble', '#58A6FF']];
const POS_ORDER = null;

async function gameView(gid, playId) {
  app.innerHTML = '<p class="muted">Loading game…</p>';
  const gm = META_GAME[gid], G = await loadGame(gid);
  if (!G.plays.length) throw new Error('No dropback plays in this game');
  const S = { pi: Math.max(0, G.plays.findIndex(p => p.id === playId)), f: 0, playing: false, speed: 1, trails: true, roles: false, auto: true, close: true, trk: null, hover: -1, cam: null };
  const first = gm.neutral ? gm.home : gm.away, second = gm.neutral ? gm.away : gm.home, sep = gm.neutral ? 'vs' : '@';
  const rolesLegend = Object.keys(ROLE_COL).map(k => `<span><span class="dot" style="background:${ROLE_COL[k]}"></span>${ROLE_NAME[k]}</span>`).join('');
  setTitle(((TEAMS[first] || {}).n || first) + ' vs ' + ((TEAMS[second] || {}).n || second));
  app.innerHTML = `
    <div class="muted small" style="margin:0 2px 8px">Week ${gm.week} · ${esc(gm.when)} · ${G.plays.length} dropback plays${gm.neutral ? ' · Neutral · ' + esc(gm.stadium || '') : ''}</div>
    <div style="display:flex;gap:8px;margin-bottom:10px"><div class="score" id="sc1"></div><div class="sit" id="sit"></div><div class="score" id="sc2"></div></div>
    <canvas id="field" style="border-radius:16px"></canvas>
    <div class="ctl2"><button class="btn" id="prev" aria-label="Previous play">⏮</button><button class="btn play" id="pl" aria-label="Play">▶</button><button class="btn" id="next" aria-label="Next play">⏭</button>
      <div style="flex:1;min-width:0"><canvas id="scrub" height="50"></canvas></div><button class="btn" id="opt" aria-label="Options">⚙</button></div>
    <div id="optpanel" class="card" style="display:none;margin-top:8px"><div class="ctl" style="margin:0 0 8px"><span class="muted small">Speed</span><select id="spd"><option value="0.5">0.5×</option><option value="1" selected>1×</option><option value="2">2×</option></select></div>
      <div class="ctl" style="margin:0"><label><input type="checkbox" id="ck-close" checked>Close-up</label><label><input type="checkbox" id="ck-trails" checked>Trails</label><label><input type="checkbox" id="ck-roles">Color by role</label><label><input type="checkbox" id="ck-auto" checked>Auto-advance</label></div></div>
    <div class="card" id="tlcard" style="margin-top:10px"><b style="font-size:15px">Game timeline</b><canvas id="tl" height="96" style="margin-top:8px"></canvas></div>
    <details class="acc" id="det"><summary id="sumhead"></summary><div class="card" style="margin-top:0"><div id="summary"></div><div id="chips" style="margin-top:8px"></div><div id="desc" class="muted small" style="margin-top:6px"></div><div class="muted small" style="margin:10px 0 4px">Timeline key</div><div class="legend" style="margin:0 0 8px">${RES_NAMES.map(r => `<span><span class="dot" style="background:${r[2]};width:8px;height:8px"></span> ${r[1]}</span>`).join('')}<span>· bar height = yards gained</span></div><div class="legend" id="legend" style="margin-top:10px"></div></div></details>`;
  const fieldCv = $('#field'), scrubCv = $('#scrub'), tlCv = $('#tl');
  const setup = (cv, h) => { const dpr = window.devicePixelRatio || 1, w = cv.clientWidth; cv.width = w * dpr; cv.height = h * dpr; cv.style.height = h + 'px'; const c = cv.getContext('2d'); c.setTransform(dpr, 0, 0, dpr, 0, 0); return [c, w, h]; };

  // ---- play data ----
  function decode(p) {
    const t = G.trk[p.id]; if (!t) return null;
    if (!t._u) {
      const bin = atob(t.b64), u8 = new Uint8Array(bin.length);
      for (let i = 0; i < bin.length; i++) u8[i] = bin.charCodeAt(i);
      t._u = new Uint16Array(u8.buffer);
    }
    return t;
  }
  function entAt(t, f, s) { const i = (f * t.ents.length + s) * 3, u = t._u; return u[i] === 0xFFFF ? null : [u[i] / 100, u[i + 1] / 100, u[i + 2] === 0xFFFF ? null : u[i + 2] / 10]; }
  function frameAt(t, f) {
    const n = t.n, ff = Math.max(0, Math.min(n - 1, f)), i = Math.floor(ff), j = Math.min(i + 1, n - 1), k = ff - i, out = [];
    for (let s = 0; s < t.ents.length; s++) {
      const a = entAt(t, i, s); if (!a) { out.push(null); continue; }
      const b = entAt(t, j, s) || a, near = k < .5 ? a : b;
      out.push([a[0] + (b[0] - a[0]) * k, a[1] + (b[1] - a[1]) * k, near[2]]);
    }
    return out;
  }
  function selectPlay(i, autoplay) {
    S.pi = Math.max(0, Math.min(G.plays.length - 1, i));
    const p = G.plays[S.pi]; S.trk = decode(p); S.f = 0; S.playing = !!autoplay && !!S.trk && S.trk.n > 1;
    S.snap = S.trk ? Math.max(0, S.trk.snap - 1) : 0;
    S.cam = S.trk ? camera(S.trk, p) : null; if (vp) sizeField();
    S.span = null;
    if (S.trk) { const e = S.trk.throw ?? S.trk.sack; if (e != null && e >= S.trk.snap) S.span = [Math.max(0, S.trk.snap - 1), Math.max(0, e - 1)]; }
    S.markers = S.trk ? S.trk.events.filter(e => EVENT[e[1]]).map(e => ({ f: Math.max(0, e[0] - 1), l: EVENT[e[1]] })).sort((a, b) => a.f - b.f) : [];
    updateText(); drawTimeline(); S.last = 0;
  }
  const home = gm.home;
  // Close-up camera: box around every player across the whole play, with a minimum size.
  function camera(t, p) {
    let x0 = 1e9, x1 = -1e9, y0 = 1e9, y1 = -1e9;
    for (let f = 0; f < t.n; f++) for (let s = 0; s < t.ents.length; s++) { if (t.ents[s][0] == null) continue; const q = entAt(t, f, s); if (!q) continue; x0 = Math.min(x0, q[0]); x1 = Math.max(x1, q[0]); y0 = Math.min(y0, q[1]); y1 = Math.max(y1, q[1]); }
    if (p.los != null) { x0 = Math.min(x0, p.los); x1 = Math.max(x1, p.los); }
    if (x0 > x1) return null;
    const fix = (a, b, min, max, m) => { a -= m; b += m; if (b - a < min) { const c = (a + b) / 2; a = c - min / 2; b = c + min / 2; } if (a < 0) { b -= a; a = 0; } if (b > max) { a -= b - max; b = max; } return [Math.max(0, a), Math.min(max, b)]; };
    const [xa, xb] = fix(x0, x1, 40, LEN, 5), [ya, yb] = fix(y0, y1, 30, WID, 4);
    return { xa, xb, ya, yb };
  }
  function updateText() {
    const p = G.plays[S.pi], sc = t => t === gm.home ? p.hs : p.as;
    const card = (t, el) => { $(el).innerHTML = `${logoHtml(t, 32)}<div class="tn" style="min-width:0"><b style="font-size:15px">${t}</b>${p.off === t ? ' <span class="dot" style="background:#3B82F6;width:8px;height:8px"></span>' : ''}</div><b class="n">${sc(t)}</b>`; };
    card(first, '#sc1'); card(second, '#sc2');
    $('#sit').innerHTML = `<b>${p.qL} ${esc(p.clock)}</b><div class="muted small">${p.off} · ${esc(p.sit)}</div>`;
    $('#sumhead').innerHTML = `<span style="display:flex;gap:10px;align-items:center"><span class="chip" style="margin:0;background:${p.col};color:${isLight(p.col) ? '#111' : '#fff'};border-color:${p.col};font-weight:700">${esc(p.res)}${p.yds != null ? ' ' + (p.yds > 0 ? '+' : '') + p.yds : ''}</span><span class="muted small" style="font-weight:500">Play ${S.pi + 1} of ${G.plays.length} · details</span></span>`;
    const chips = [];
    [p.form, p.pers, p.box != null ? p.box + ' in box' : null, p.cov ? p.cov + ' · ' + (p.covType || '') : null, p.drop, p.pa ? 'Play action' : null, p.ttt != null ? p.ttt.toFixed(1) + ' s to throw' : null].forEach(x => x && chips.push(`<span class="chip">${esc(x)}</span>`));
    $('#chips').innerHTML = chips.join(''); $('#desc').textContent = p.desc;
    legend();
  }
  function legend() {
    const dot = (f, s, l) => `<span><span class="dot" style="background:${f};border:2px solid ${s};width:12px;height:12px;vertical-align:-2px"></span>${l}</span>`;
    const lines = `<span><span style="display:inline-block;width:14px;height:3px;background:#3B82F6;vertical-align:3px;margin-right:4px"></span>Line of scrimmage</span><span><span style="display:inline-block;width:14px;height:3px;background:#FACC15;vertical-align:3px;margin-right:4px"></span>Line to gain</span>`;
    const side = S.roles ? rolesLegend : dot(teamCol(home), '#fff', home + (gm.neutral ? '' : ' (home)')) + dot('#fff', teamCol(gm.away), gm.away + (gm.neutral ? '' : ' (away)'));
    $('#legend').innerHTML = side + dot('#8B4A1C', '#8B4A1C', 'Ball') + lines;
  }

  // ---- field drawing ----
  let vp = null;
  function fit(W, H, portrait, r) {
    const ax = r.xb - r.xa, ay = r.yb - r.ya, aspect = portrait ? ay / ax : ax / ay;
    let w, h; if (W / H > aspect) { h = H; w = H * aspect; } else { w = W; h = W / aspect; }
    return { L: (W - w) / 2, T: (H - h) / 2, w, h, portrait, r, sc: portrait ? w / ay : w / ax };
  }
  const P = (x, y) => { const r = vp.r, u = (x - r.xa) / (r.xb - r.xa), v = (y - r.ya) / (r.yb - r.ya); return vp.portrait ? [vp.L + v * vp.w, vp.T + (1 - u) * vp.h] : [vp.L + u * vp.w, vp.T + (1 - v) * vp.h]; };
  const ENT_R = () => Math.max(1.1 * vp.sc, 7);
  function drawField(c, W, H) {
    c.fillStyle = '#1D5A28'; c.beginPath(); c.roundRect(0, 0, W, H, 16); c.fill();
    c.beginPath(); c.rect(vp.L, vp.T, vp.w, vp.h); c.clip();
    const p = G.plays[S.pi], sc = vp.sc, rot = vp.portrait ? -Math.PI / 2 : 0;
    const rect = (x0, x1, y0, y1, col) => { const a = P(x0, y0), b = P(x1, y1); c.fillStyle = col; c.fillRect(Math.min(a[0], b[0]), Math.min(a[1], b[1]), Math.abs(a[0] - b[0]), Math.abs(a[1] - b[1])); };
    const line = (a, b, col, w) => { const s1 = P(a[0], a[1]), s2 = P(b[0], b[1]); c.strokeStyle = col; c.lineWidth = w; c.beginPath(); c.moveTo(s1[0], s1[1]); c.lineTo(s2[0], s2[1]); c.stroke(); };
    const text = (t, x, y, size, col, ang, weight = '') => { const s = P(x, y); c.save(); c.translate(s[0], s[1]); c.rotate(ang); c.fillStyle = col; c.font = `${weight} ${size}px system-ui,sans-serif`; c.textAlign = 'center'; c.textBaseline = 'middle'; c.fillText(t, 0, 0); c.restore(); };
    for (let i = 0; i < 20; i++) rect(10 + i * 5, 15 + i * 5, 0, WID, i % 2 ? '#2F843C' : '#2B7A37');
    [[0, p.off, -Math.PI / 2], [110, p.def, Math.PI / 2]].forEach(([x0, team, a]) => {
      const col = teamCol(team); rect(x0, x0 + 10, 0, WID, col);
      const ctr = P(x0 + 5, WID / 2); c.fillStyle = 'rgba(255,255,255,.92)'; c.beginPath(); c.arc(ctr[0], ctr[1], 4.65 * sc, 0, 7); c.fill();
      drawLogo(c, team, ctr[0], ctr[1], 4.0 * sc);
      const tc = isLight(col) ? 'rgba(0,0,0,.7)' : 'rgba(255,255,255,.9)';
      [WID / 2 - 15, WID / 2 + 15].forEach(y => text(team, x0 + 5, y, 3 * sc, tc, a + rot, '700'));
    });
    if (!gm.neutral) { const s = P(60, WID / 2); drawLogo(c, gm.home, s[0], s[1], 5.5 * sc, .9); }
    for (let i = 0; i <= 18; i++) line([15 + i * 5, 0], [15 + i * 5, WID], 'rgba(255,255,255,.8)', 1);
    for (let x = 11; x < 110; x++) { if (x % 5 === 0) continue; HASHES.forEach((y, k) => { const y0 = k === 3 ? y - .67 : k === 0 ? y : y - .33; line([x, y0], [x, y0 + .67], 'rgba(255,255,255,.8)', 1); }); }
    line([10, 0], [10, WID], 'rgba(255,255,255,.8)', 2); line([110, 0], [110, WID], 'rgba(255,255,255,.8)', 2);
    const a0 = P(0, 0), a1 = P(LEN, WID); c.strokeStyle = 'rgba(255,255,255,.8)'; c.lineWidth = 2; c.strokeRect(Math.min(a0[0], a1[0]), Math.min(a0[1], a1[1]), Math.abs(a0[0] - a1[0]), Math.abs(a0[1] - a1[1]));
    for (let k = 1; k <= 9; k++) { const x = 10 + 10 * k, n = k <= 5 ? 10 * k : 100 - 10 * k; text(n, x, 12, 2.2 * sc, 'rgba(255,255,255,.75)', rot); text(n, x, WID - 12, 2.2 * sc, 'rgba(255,255,255,.75)', rot + Math.PI); }
    const lw = Math.max(.3 * sc, 2);
    if (p.los != null) line([p.los, 0], [p.los, WID], '#3B82F6', lw);
    if (p.fd != null) line([p.fd, 0], [p.fd, WID], '#FACC15', lw);
  }
  function look(team, role) {
    if (S.roles) { const c = ROLE_COL[role] || '#9CA3AF'; return { fill: c, stroke: '#0D1117', text: '#0D1117', trail: c }; }
    const col = teamCol(team), light = isLight(col);
    if (team === home) return { fill: col, stroke: '#fff', text: onColor(col), trail: col };
    const trim = light ? '#333333' : col; return { fill: '#fff', stroke: trim, text: light ? '#111111' : col, trail: trim };
  }
  function entityPoints() {
    const t = S.trk; if (!t) return [];
    return frameAt(t, S.f).map((q, s) => q && t.ents[s][0] ? { id: t.ents[s][0], s, xy: P(q[0], q[1]) } : null).filter(Boolean);
  }
  function drawEntities(c) {
    const t = S.trk;
    if (!t) { c.fillStyle = '#fff'; c.font = '15px system-ui'; c.textAlign = 'center'; c.fillText('No tracking for this play', fieldCv.clientWidth / 2, 80); return; }
    const sc = vp.sc, r = ENT_R(), fr = frameAt(t, S.f);
    if (S.trails) {
      const last = Math.floor(S.f);
      t.ents.forEach((e, s) => {
        const ball = e[0] == null, col = ball ? '#F5DEB3' : look(e[2], e[3]).trail; c.strokeStyle = col; c.globalAlpha = .6; c.lineWidth = 1.5; c.beginPath(); let started = false;
        for (let f = 0; f <= last && f < t.n; f++) { const q = entAt(t, f, s); if (!q) continue; const [x, y] = P(q[0], q[1]); started ? c.lineTo(x, y) : c.moveTo(x, y); started = true; }
        if (fr[s]) { const [x, y] = P(fr[s][0], fr[s][1]); started ? c.lineTo(x, y) : c.moveTo(x, y); }
        c.stroke(); c.globalAlpha = 1;
      });
    }
    let ball = null;
    t.ents.forEach((e, s) => {
      const q = fr[s]; if (!q) return;
      if (e[0] == null) { ball = [s, q]; return; }
      const [x, y] = P(q[0], q[1]), lk = look(e[2], e[3]);
      if (q[2] != null) { const a = q[2] * Math.PI / 180, dx = Math.sin(a), dy = Math.cos(a), d = vp.portrait ? [dy, -dx] : [dx, -dy]; c.strokeStyle = 'rgba(255,255,255,.9)'; c.lineWidth = 2; c.beginPath(); c.moveTo(x, y); c.lineTo(x + d[0] * r * 1.7, y + d[1] * r * 1.7); c.stroke(); }
      c.fillStyle = lk.fill; c.beginPath(); c.arc(x, y, r, 0, 7); c.fill(); c.strokeStyle = lk.stroke; c.lineWidth = 2; c.stroke();
      if (S.hover === e[0]) { c.strokeStyle = '#D50A0A'; c.lineWidth = 3; c.beginPath(); c.arc(x, y, r + 3, 0, 7); c.stroke(); }
      if (e[1] != null && r >= 6) { c.fillStyle = lk.text; c.font = `700 ${r * 1.0}px system-ui`; c.textAlign = 'center'; c.textBaseline = 'middle'; c.fillText(e[1], x, y + 1); }
    });
    if (ball) {
      const [s, q] = ball, prev = frameAt(t, Math.max(0, S.f - 1))[s], [x, y] = P(q[0], q[1]);
      let ang = vp.portrait ? -Math.PI / 2 : 0;
      if (prev) { const [px, py] = P(prev[0], prev[1]), dx = x - px, dy = y - py; if (dx * dx + dy * dy > .01) ang = Math.atan2(dy, dx); }
      c.save(); c.translate(x, y); c.rotate(ang); c.fillStyle = '#8B4A1C'; c.strokeStyle = '#fff'; c.lineWidth = 1; c.beginPath(); c.ellipse(0, 0, Math.max(.6 * sc, 4), Math.max(.36 * sc, 2.5), 0, 0, 7); c.fill(); c.stroke(); c.restore();
    }
  }
  function sizeField() {
    const w = fieldCv.clientWidth, portrait = w < 700, r = (S.close && S.cam) ? S.cam : { xa: 0, xb: LEN, ya: 0, yb: WID };
    const ax = r.xb - r.xa, ay = r.yb - r.ya, ideal = portrait ? w * ax / ay : w * ay / ax;
    const h = Math.round(Math.min(ideal, window.innerHeight * (portrait ? .66 : .72)));
    const [c] = setup(fieldCv, h); vp = fit(w, h, portrait, r); return c;
  }
  function render() {
    const c = fieldCv.getContext('2d'), W = fieldCv.clientWidth, H = parseFloat(fieldCv.style.height);
    c.save(); drawField(c, W, H); drawEntities(c); c.restore();
    if (S.trk) { const t = (S.f - S.snap) / FPS, txt = (t >= 0 ? '+' : '') + t.toFixed(1) + 's', w = 54; c.fillStyle = 'rgba(6,11,22,.62)'; c.beginPath(); c.roundRect(10, 10, w, 24, 12); c.fill(); c.fillStyle = '#fff'; c.font = '600 13px system-ui'; c.textAlign = 'center'; c.textBaseline = 'middle'; c.fillText(txt, 10 + w / 2, 22.5); }
    drawScrub(); $('#pl').textContent = S.playing ? '⏸' : '▶';
  }
  function drawScrub() {
    const [c, W, H] = setup(scrubCv, 50); const t = S.trk, n = t ? Math.max(t.n - 1, 1) : 1, x0 = 12, x1 = W - 12, ty = 14;
    const fx = f => x0 + f / n * (x1 - x0);
    if (S.span) { c.fillStyle = '#013369'; c.fillRect(fx(S.span[0]), ty - 8, fx(S.span[1]) - fx(S.span[0]), 16); }
    c.fillStyle = '#2A3B5C'; c.fillRect(x0, ty - 3, x1 - x0, 6); c.fillStyle = '#D50A0A'; c.fillRect(x0, ty - 3, fx(S.f) - x0, 6);
    let lastx = -99, row = 0; c.font = '11px system-ui'; c.textAlign = 'center'; c.textBaseline = 'top';
    S.markers.forEach(m => { const x = fx(m.f); row = x - lastx < 40 ? 1 - row : 0; lastx = x; c.strokeStyle = '#9AA8BD'; c.lineWidth = 1.5; c.beginPath(); c.moveTo(x, ty - 8); c.lineTo(x, ty + 9); c.stroke(); c.fillStyle = '#9AA8BD'; c.fillText(m.l, x, ty + 12 + row * 13); });
    c.fillStyle = '#D50A0A'; c.beginPath(); c.arc(fx(S.f), ty, 9, 0, 7); c.fill(); c.strokeStyle = '#fff'; c.lineWidth = 1.5; c.stroke();
    S.scrubGeom = { x0, x1, n };
  }
  function drawTimeline() {
    if (!tlCv.clientWidth) return;
    const [c, W, H] = setup(tlCv, 96), plays = G.plays, left = 34, gap = 8, mid = 58, unit = (W - left - gap * 3) / plays.length;
    drawLogo(c, home, 14, mid - 22, 10); drawLogo(c, gm.away, 14, mid + 22, 10);
    c.strokeStyle = '#2A3B5C'; c.beginPath(); c.moveTo(left, mid); c.lineTo(W, mid); c.stroke();
    let x = left, i = 0; S.bars = [];
    while (i < plays.length) {
      const q = plays[i].q; let j = i; while (j < plays.length && plays[j].q === q) j++;
      c.fillStyle = '#9AA8BD'; c.font = '11px system-ui'; c.textAlign = 'left'; c.textBaseline = 'top'; c.fillText(plays[i].qL, x, 0);
      for (let k = i; k < j; k++) {
        const p = plays[k], bh = 3 + Math.min(27, Math.abs(p.yds || 0)), bw = Math.max(unit - 1, 1.5);
        if (k === S.pi) { c.fillStyle = 'rgba(213,10,10,.35)'; c.fillRect(x - 1, 14, bw + 2, H - 14); }
        c.fillStyle = p.col || '#9CA3AF'; p.off === home ? c.fillRect(x, mid - bh, bw, bh) : c.fillRect(x, mid, bw, bh);
        S.bars.push([x, x + unit, k]); x += unit;
      }
      x += gap; i = j;
    }
  }

  // ---- interaction ----
  const evt = (e, el) => { const r = el.getBoundingClientRect(); const t = e.touches ? e.touches[0] || e.changedTouches[0] : e; return [t.clientX - r.left, t.clientY - r.top]; };
  const pickPlayer = ([x, y]) => { const r = ENT_R() + 12; let best = null, bd = 1e9; entityPoints().forEach(p => { const d = Math.hypot(p.xy[0] - x, p.xy[1] - y); if (d <= r && d < bd) { best = p; bd = d; } }); return best; };
  fieldCv.addEventListener('click', e => {
    const p = pickPlayer(evt(e, fieldCv)); if (!p) return;
    const ent = S.trk.ents[p.s]; const team = ent[2];
    go(`player/${p.id}/${team}`);
  });
  fieldCv.addEventListener('mousemove', e => { const p = pickPlayer(evt(e, fieldCv)); S.hover = p ? p.id : -1; fieldCv.title = p ? ((G.info[p.id] || [])[0] || '') + ' · ' + ((G.info[p.id] || [])[1] || '') + ' — tap for stats' : ''; });
  let drag = false;
  const scrubTo = e => { const [x] = evt(e, scrubCv), g = S.scrubGeom; if (!g || !S.trk) return; S.playing = false; S.f = Math.max(0, Math.min(S.trk.n - 1, (x - g.x0) / (g.x1 - g.x0) * g.n)); };
  scrubCv.addEventListener('pointerdown', e => { drag = true; scrubCv.setPointerCapture(e.pointerId); scrubTo(e); });
  scrubCv.addEventListener('pointermove', e => { if (drag) scrubTo(e); });
  scrubCv.addEventListener('pointerup', () => drag = false);
  tlCv.addEventListener('click', e => { const [x] = evt(e, tlCv), b = S.bars.find(b => x >= b[0] && x < b[1]); if (b) selectPlay(b[2], true); });
  $('#pl').onclick = () => { if (!S.trk) return; if (!S.playing && S.f >= S.trk.n - 1) S.f = 0; S.playing = !S.playing; };
  $('#prev').onclick = () => selectPlay(S.pi - 1, true); $('#next').onclick = () => selectPlay(S.pi + 1, true);
  $('#spd').onchange = e => S.speed = +e.target.value;
  $('#opt').onclick = () => { const o = $('#optpanel'); o.style.display = o.style.display === 'none' ? 'block' : 'none'; };
  $('#ck-close').onchange = e => { S.close = e.target.checked; sizeField(); };
  $('#ck-trails').onchange = e => S.trails = e.target.checked; $('#ck-roles').onchange = e => { S.roles = e.target.checked; legend(); }; $('#ck-auto').onchange = e => S.auto = e.target.checked;
  let raf = 0, alive = true, lastT = 0;
  function loop(ts) {
    if (!alive) return; const dt = Math.min(.1, (ts - (lastT || ts)) / 1000); lastT = ts;
    if (S.playing && S.trk) {
      S.f = Math.min(S.trk.n - 1, S.f + dt * FPS * S.speed);
      if (S.f >= S.trk.n - 1) { S.playing = false; if (S.auto && S.pi + 1 < G.plays.length) selectPlay(S.pi + 1, true); }
    }
    render(); raf = requestAnimationFrame(loop);
  }
  const onResize = () => { sizeField(); drawTimeline(); };
  window.addEventListener('resize', onResize);
  stopViewer = () => { alive = false; cancelAnimationFrame(raf); window.removeEventListener('resize', onResize); };
  sizeField(); selectPlay(S.pi, false); raf = requestAnimationFrame(loop);
}

route();
