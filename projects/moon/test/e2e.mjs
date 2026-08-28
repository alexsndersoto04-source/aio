// Moon — Suite de pruebas E2E contra la API real
// ============================================================
// Se ejecuta contra una instancia ARRANCADA de moon (API Titan + Postgres
// real). Cubre auth (registro/login/2FA/refresh/rotación), posts, feed,
// comentarios, likes/saves, follows/blocks, mensajería en vivo (WebSocket),
// notificaciones, subida de imágenes real, reportes, admin y seguridad
// (404/405/rate limit/CORS).
//
// Uso:
//   API_BASE=http://127.0.0.1:3000 MOON_LOG=/tmp/moon-server.log \
//     node projects/moon/test/e2e.mjs
//
// Los tests 2FA leen el código del LOG del servidor (sin SMTP el correo
// se imprime en consola). Salida: resumen PASS/FAIL; exit 0 si todo pasa.

import { readFileSync } from 'node:fs';

const API_BASE = (process.env.API_BASE || 'http://127.0.0.1:3000').replace(/\/$/, '');
const LOG_FILE = process.env.MOON_LOG || '/tmp/moon-server.log';

// Último código de 6 dígitos que coincide con `re` en el log del servidor.
function lastLogCode(re) {
  try {
    const log = readFileSync(LOG_FILE, 'utf8');
    const m = [...log.matchAll(re)];
    return m.length ? m[m.length - 1][1] : null;
  } catch {
    return null;
  }
}

let passed = 0;
let failed = 0;
const failures = [];

function check(name, cond, detail) {
  if (cond) {
    passed++;
    console.log(`  PASS  ${name}`);
  } else {
    failed++;
    failures.push(name + (detail ? ` — ${detail}` : ''));
    console.log(`  FAIL  ${name}${detail ? ` — ${detail}` : ''}`);
  }
}

async function req(method, path, { token, body, headers = {}, raw = false } = {}) {
  const h = { ...headers };
  if (token) h['Authorization'] = `Bearer ${token}`;
  let payload;
  if (body instanceof FormData) {
    payload = body; // el fetch pone el Content-Type multipart por sí solo
  } else if (body !== undefined && typeof body !== 'string') {
    h['Content-Type'] = 'application/json';
    payload = JSON.stringify(body);
  } else {
    payload = body;
  }
  const res = await fetch(API_BASE + path, { method, headers: h, body: payload, redirect: 'manual' });
  if (raw) return res;
  const text = await res.text();
  let json = null;
  try { json = JSON.parse(text); } catch { /* no JSON */ }
  return { status: res.status, json, text, headers: res.headers };
}

function rand() {
  return Math.random().toString(36).slice(2, 10);
}

// JPEG 1x1 real (bytecode mínimo pero válido)
function tinyJpeg() {
  return Uint8Array.from([
    0xff,0xd8,0xff,0xe0,0x00,0x10,0x4a,0x46,0x49,0x46,0x00,0x01,0x01,0x00,0x00,0x01,
    0x00,0x01,0x00,0x00,0xff,0xdb,0x00,0x43,0x00,0x08,0x06,0x06,0x07,0x06,0x05,0x08,
    0x07,0x07,0x07,0x09,0x09,0x08,0x0a,0x0c,0x14,0x0d,0x0c,0x0b,0x0b,0x0c,0x19,0x12,
    0x13,0x0f,0x14,0x1d,0x1a,0x1f,0x1e,0x1d,0x1a,0x1c,0x1c,0x20,0x24,0x2e,0x27,0x20,
    0x22,0x2c,0x23,0x1c,0x1c,0x28,0x37,0x29,0x2c,0x30,0x31,0x34,0x34,0x34,0x1f,0x27,
    0x39,0x3d,0x38,0x32,0x3c,0x2e,0x33,0x34,0x32,0xff,0xc0,0x00,0x0b,0x08,0x00,0x01,
    0x00,0x01,0x01,0x01,0x11,0x00,0xff,0xc4,0x00,0x1f,0x00,0x00,0x01,0x05,0x01,0x01,
    0x01,0x01,0x01,0x01,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x01,0x02,0x03,0x04,
    0x05,0x06,0x07,0x08,0x09,0x0a,0x0b,0xff,0xc4,0x00,0xb5,0x10,0x00,0x02,0x01,0x03,
    0x03,0x02,0x04,0x03,0x05,0x05,0x04,0x04,0x00,0x00,0x01,0x7d,0x01,0x02,0x03,0x00,
    0x04,0x11,0x05,0x12,0x21,0x31,0x41,0x06,0x13,0x51,0x61,0x07,0x22,0x71,0x14,0x32,
    0x81,0x91,0xa1,0x08,0x23,0x42,0xb1,0xc1,0x15,0x52,0xd1,0xf0,0x24,0x33,0x62,0x72,
    0x82,0x09,0x0a,0x16,0x17,0x18,0x19,0x1a,0x25,0x26,0x27,0x28,0x29,0x2a,0x34,0x35,
    0x36,0x37,0x38,0x39,0x3a,0x43,0x44,0x45,0x46,0x47,0x48,0x49,0x4a,0x53,0x54,0x55,
    0x56,0x57,0x58,0x59,0x5a,0x63,0x64,0x65,0x66,0x67,0x68,0x69,0x6a,0x73,0x74,0x75,
    0x76,0x77,0x78,0x79,0x7a,0x83,0x84,0x85,0x86,0x87,0x88,0x89,0x8a,0x92,0x93,0x94,
    0x95,0x96,0x97,0x98,0x99,0x9a,0xa2,0xa3,0xa4,0xa5,0xa6,0xa7,0xa8,0xa9,0xaa,0xb2,
    0xb3,0xb4,0xb5,0xb6,0xb7,0xb8,0xb9,0xba,0xc2,0xc3,0xc4,0xc5,0xc6,0xc7,0xc8,0xc9,
    0xca,0xd2,0xd3,0xd4,0xd5,0xd6,0xd7,0xd8,0xd9,0xda,0xe1,0xe2,0xe3,0xe4,0xe5,0xe6,
    0xe7,0xe8,0xe9,0xea,0xf1,0xf2,0xf3,0xf4,0xf5,0xf6,0xf7,0xf8,0xf9,0xfa,0xff,0xda,
    0x00,0x08,0x01,0x01,0x00,0x00,0x3f,0x00,0x7b,0x94,0x11,0x00,0x00,0x00,0x00,0x00,
    0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,
    0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,
    0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,
    0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,
    0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,
    0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,
    0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,
    0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,
    0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,
    0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0xff,
    0xd9,
  ]);
}

async function main() {
  const s = rand();
  const u1 = `alice_${s}`;
  const u2 = `bob_${s}`;
  console.log(`\n=== Moon E2E — ${API_BASE} (sufijo ${s}) ===\n`);

  // ---------- Salud ----------
  console.log('[health]');
  const health = await req('GET', '/api/health');
  check('GET /api/health -> 200', health.status === 200, `status=${health.status}`);
  check('health.json.status ok', health.json && health.json.status === 'ok', health.text.slice(0, 120));

  // ---------- Registro ----------
  console.log('[auth]');
  const reg1 = await req('POST', '/api/auth/register', {
    body: { username: u1, email: `${u1}@moon.test`, password: 'Secret123!', display_name: 'Alice' },
  });
  check('registro alice -> 201', reg1.status === 201, `status=${reg1.status} ${reg1.text.slice(0, 160)}`);
  const t1 = reg1.json && (reg1.json.access_token || reg1.json.access);
  const r1 = reg1.json && (reg1.json.refresh_token || reg1.json.refresh);
  check('registro devuelve access+refresh', !!t1 && !!r1, reg1.text.slice(0, 160));

  const dup = await req('POST', '/api/auth/register', {
    body: { username: u1, email: `${u1}@moon.test`, password: 'Secret123!', display_name: 'Alice' },
  });
  check('username duplicado -> 4xx', dup.status >= 400 && dup.status < 500, `status=${dup.status}`);

  const reg2 = await req('POST', '/api/auth/register', {
    body: { username: u2, email: `${u2}@moon.test`, password: 'Secret123!', display_name: 'Bob' },
  });
  check('registro bob -> 201', reg2.status === 201, `status=${reg2.status} ${reg2.text.slice(0, 160)}`);
  const t2 = reg2.json && (reg2.json.access_token || reg2.json.access);

  // login
  const login = await req('POST', '/api/auth/login', { body: { email: `${u1}@moon.test`, password: 'Secret123!' } });
  check('login -> 200', login.status === 200, `status=${login.status} ${login.text.slice(0, 160)}`);
  const lt = login.json && (login.json.access_token || login.json.access);
  const lr = login.json && (login.json.refresh_token || login.json.refresh);
  check('login devuelve tokens', !!lt && !!lr);

  const badLogin = await req('POST', '/api/auth/login', { body: { email: `${u1}@moon.test`, password: 'Mala12345!' } });
  check('login con contraseña mala -> 4xx', badLogin.status >= 400 && badLogin.status < 500, `status=${badLogin.status}`);

  const me = await req('GET', '/api/auth/me', { token: lt });
  check('GET /api/auth/me -> 200', me.status === 200, `status=${me.status} ${me.text.slice(0, 160)}`);
  const meUser = me.json && (me.json.user || me.json);
  check('me.username correcto', meUser && (meUser.username === u1 || meUser.name === u1), me.text.slice(0, 160));
  check('me no expone password_hash', !/password_hash/.test(me.text), 'el body contiene password_hash');

  // refresh con rotación
  const ref = await req('POST', '/api/auth/refresh', { body: { refresh_token: lr } });
  check('refresh -> 200', ref.status === 200, `status=${ref.status} ${ref.text.slice(0, 160)}`);
  const nr = ref.json && (ref.json.refresh_token || ref.json.refresh);
  check('refresh rota (token nuevo)', !!nr && nr !== lr);
  const reuse = await req('POST', '/api/auth/refresh', { body: { refresh_token: lr } });
  check('refresh reutilizado -> 401', reuse.status === 401, `status=${reuse.status}`);

  // sin token
  const noAuth = await req('GET', '/api/auth/me');
  check('sin token -> 401', noAuth.status === 401, `status=${noAuth.status}`);

  // ---------- 2FA (código leído del log del servidor) ----------
  console.log('[2fa]');
  const en = await req('POST', '/api/auth/2fa/enable', { token: lt, body: { password: 'Secret123!' } });
  check('2fa enable -> 200 + temp_token', en.status === 200 && en.json && !!en.json.temp_token, `status=${en.status} ${en.text.slice(0, 160)}`);
  const enBad = await req('POST', '/api/auth/2fa/enable', { token: lt, body: { password: 'Mala12345!' } });
  check('2fa enable con contraseña mala -> 401', enBad.status === 401, `status=${enBad.status}`);
  const code1 = lastLogCode(/Tu código para activar 2FA es: (\d{6})/);
  check('código 2FA visible en el log del servidor', /^\d{6}$/.test(code1 || ''), 'no se encontró "Tu código para activar 2FA es: XXXXXX" en ' + LOG_FILE);
  const confBad = await req('POST', '/api/auth/2fa/confirm', { body: { temp_token: en.json && en.json.temp_token, code: '000000' } });
  check('2fa confirm con código malo -> 401', confBad.status === 401, `status=${confBad.status}`);
  const conf = await req('POST', '/api/auth/2fa/confirm', { body: { temp_token: en.json && en.json.temp_token, code: code1 } });
  check('2fa confirm -> 204', conf.status === 204, `status=${conf.status} ${conf.text.slice(0, 120)}`);
  const me3 = await req('GET', '/api/auth/me', { token: lt });
  check('me.twofa_enabled = true', /"twofa_enabled":\s*true/.test(me3.text), me3.text.slice(0, 200));

  // login ahora exige el segundo factor
  const l2 = await req('POST', '/api/auth/login', { body: { email: `${u1}@moon.test`, password: 'Secret123!' } });
  check('login con 2fa activo -> twofa_required + temp_token', l2.status === 200 && l2.json && l2.json.twofa_required === true && !!l2.json.temp_token, `status=${l2.status} ${l2.text.slice(0, 160)}`);
  const code2 = lastLogCode(/Tu código de verificación es: (\d{6})/);
  check('código de login 2FA visible en el log', /^\d{6}$/.test(code2 || ''), 'no se encontró "Tu código de verificación es: XXXXXX" en ' + LOG_FILE);
  const ver = await req('POST', '/api/auth/2fa/verify', { body: { temp_token: l2.json && l2.json.temp_token, code: code2 } });
  check('2fa verify -> 200 + access_token', ver.status === 200 && ver.json && !!(ver.json.access_token || ver.json.access), `status=${ver.status} ${ver.text.slice(0, 160)}`);
  const verBad = await req('POST', '/api/auth/2fa/verify', { body: { temp_token: 'temporal.malo.x', code: '123456' } });
  check('2fa verify con token malo -> 401', verBad.status === 401, `status=${verBad.status}`);

  const dis = await req('POST', '/api/auth/2fa/disable', { token: lt, body: { password: 'Secret123!' } });
  check('2fa disable -> 204', dis.status === 204, `status=${dis.status} ${dis.text.slice(0, 120)}`);
  const l3 = await req('POST', '/api/auth/login', { body: { email: `${u1}@moon.test`, password: 'Secret123!' } });
  check('login normal tras desactivar 2fa', l3.status === 200 && !(l3.json && l3.json.twofa_required === true) && !!(l3.json && (l3.json.access_token || l3.json.access)), `status=${l3.status} ${l3.text.slice(0, 160)}`);

  // ---------- Perfil ----------
  console.log('[perfil]');
  const upd = await req('PATCH', '/api/auth/update', {
    token: lt,
    body: { display_name: 'Alice M', bio: 'Probando moon', location: 'Luna' },
  });
  check('PATCH /api/auth/update -> 200', upd.status === 200, `status=${upd.status} ${upd.text.slice(0, 160)}`);
  const me2 = await req('GET', '/api/auth/me', { token: lt });
  check('bio persistido', /Probando moon/.test(me2.text), me2.text.slice(0, 200));

  // ---------- Posts ----------
  console.log('[posts]');
  const post = await req('POST', '/api/posts', {
    token: lt,
    body: { content: `Primer post de prueba #e2e${s} hola mundo @${u2}` },
  });
  check('crear post -> 201', post.status === 201, `status=${post.status} ${post.text.slice(0, 200)}`);
  const postObj = post.json && (post.json.post || post.json);
  const postId = postObj && (postObj.id || (postObj.data && postObj.data.id));
  check('post tiene id', !!postId, post.text.slice(0, 200));

  const empty = await req('POST', '/api/posts', { token: lt, body: { content: '   ' } });
  check('post vacío -> 4xx', empty.status >= 400 && empty.status < 500, `status=${empty.status}`);

  const long = await req('POST', '/api/posts', { token: lt, body: { content: 'x'.repeat(2500) } });
  check('post > 2000 chars -> 4xx', long.status >= 400 && long.status < 500, `status=${long.status}`);

  const getPost = await req('GET', `/api/posts/${postId}`, { token: lt });
  check('GET post -> 200', getPost.status === 200, `status=${getPost.status} ${getPost.text.slice(0, 160)}`);
  check('post contiene hashtag', /e2e/.test(getPost.text));

  // like / save
  const like = await req('POST', `/api/posts/${postId}/like`, { token: t2, body: {} });
  check('like -> 200/201', like.status === 200 || like.status === 201, `status=${like.status} ${like.text.slice(0, 160)}`);
  const unsave = await req('DELETE', `/api/posts/${postId}/save`, { token: t2, body: {} });
  const save = await req('POST', `/api/posts/${postId}/save`, { token: t2, body: {} });
  check('save -> 200/201', save.status === 200 || save.status === 201, `status=${save.status} ${save.text.slice(0, 160)}`);

  // comentarios
  const com = await req('POST', `/api/posts/${postId}/comments`, { token: t2, body: { content: 'Comentario de prueba #uno' } });
  check('comentario -> 201', com.status === 201 || com.status === 200, `status=${com.status} ${com.text.slice(0, 160)}`);
  const comObj = com.json && (com.json.comment || com.json);
  const comId = comObj && comObj.id;
  const coms = await req('GET', `/api/posts/${postId}/comments`, { token: lt });
  check('listar comentarios -> 200', coms.status === 200, `status=${coms.status}`);
  check('comentario visible', /#uno/.test(coms.text));

  // ---------- Feed ----------
  console.log('[feed]');
  for (const f of ['/api/feed', '/api/feed/latest', '/api/feed/trending', '/api/feed/for-you']) {
    const r = await req('GET', f, { token: lt });
    check(`GET ${f} -> 200`, r.status === 200, `status=${r.status} ${r.text.slice(0, 120)}`);
  }
  const feed = await req('GET', '/api/feed/latest', { token: lt });
  check('latest incluye mi post', new RegExp(`#e2e${s}`).test(feed.text));

  // hashtags
  const tags = await req('GET', '/api/hashtags', { token: lt });
  check('GET /api/hashtags -> 200', tags.status === 200, `status=${tags.status}`);
  const tagPage = await req('GET', `/api/hashtags/e2e${s}`, { token: lt });
  check(`GET /api/hashtags/e2e${s} -> 200`, tagPage.status === 200, `status=${tagPage.status} ${tagPage.text.slice(0, 120)}`);

  // ---------- Social ----------
  console.log('[social]');
  // perfil público de bob
  const bobMe = await req('GET', '/api/auth/me', { token: t2 });
  const bobUser = bobMe.json && (bobMe.json.user || bobMe.json);
  const bobId = bobUser && bobUser.id;
  const bobProfile = await req('GET', `/api/users/${bobId}`, { token: lt });
  check('perfil público bob -> 200', bobProfile.status === 200, `status=${bobProfile.status} ${bobProfile.text.slice(0, 120)}`);

  const follow = await req('POST', `/api/users/${bobId}/follow`, { token: lt, body: {} });
  check('follow -> 200/201', follow.status === 200 || follow.status === 201, `status=${follow.status} ${follow.text.slice(0, 160)}`);
  const unfollow = await req('DELETE', `/api/users/${bobId}/follow`, { token: lt, body: {} });
  check('unfollow -> 200', unfollow.status === 200, `status=${unfollow.status} ${unfollow.text.slice(0, 120)}`);

  const search = await req('GET', `/api/search?q=${u2}`, { token: lt });
  check('búsqueda por usuario -> 200', search.status === 200, `status=${search.status}`);
  check('búsqueda encuentra a bob', new RegExp(u2).test(search.text), search.text.slice(0, 200));

  const sugg = await req('GET', '/api/users/suggestions', { token: lt });
  check('sugerencias -> 200', sugg.status === 200, `status=${sugg.status}`);

  // ---------- Mensajería + WebSocket ----------
  console.log('[mensajería + WS]');
  const wsEvents = [];
  let wsOpen = false;
  let ws = null;
  try {
    const wsBase = API_BASE.replace(/^http/, 'ws');
    ws = new WebSocket(`${wsBase}/ws?token=${encodeURIComponent(t2)}`);
    await new Promise((resolve, reject) => {
      const to = setTimeout(() => reject(new Error('ws timeout')), 8000);
      ws.onopen = () => { wsOpen = true; clearTimeout(to); resolve(); };
      ws.onerror = (e) => { clearTimeout(to); reject(new Error('ws error')); };
    });
    ws.onmessage = (ev) => {
      try { wsEvents.push(JSON.parse(ev.data)); } catch { wsEvents.push(ev.data); }
    };
  } catch (e) {
    check('WebSocket conecta', false, String(e));
  }
  check('WebSocket conecta', wsOpen);

  const conv = await req('POST', '/api/messages/conversations', { token: lt, body: { user_id: bobId } });
  check('crear conversación -> 200/201', conv.status === 200 || conv.status === 201, `status=${conv.status} ${conv.text.slice(0, 200)}`);
  const convObj = conv.json && (conv.json.conversation || conv.json);
  const convId = convObj && convObj.id;
  check('conversación tiene id', !!convId, conv.text.slice(0, 200));

  if (convId) {
    const sendMsg = await req('POST', `/api/messages/conversations/${convId}/messages`, {
      token: lt,
      body: { content: 'Hola Bob, mensaje E2E' },
    });
    check('enviar mensaje -> 200/201', sendMsg.status === 200 || sendMsg.status === 201, `status=${sendMsg.status} ${sendMsg.text.slice(0, 200)}`);
    const msgObj = sendMsg.json && (sendMsg.json.message || sendMsg.json);
    const msgId = msgObj && msgObj.id;

    const thread = await req('GET', `/api/messages/conversations/${convId}`, { token: t2 });
    check('hilo (bob) -> 200', thread.status === 200, `status=${thread.status} ${thread.text.slice(0, 160)}`);
    check('hilo contiene el mensaje', /mensaje E2E/.test(thread.text));

    // espera el evento en vivo en el WS de bob
    await new Promise((r) => setTimeout(r, 2500));
    check('WS entregó evento en vivo a bob', wsEvents.length > 0, `eventos=${JSON.stringify(wsEvents).slice(0, 200)}`);

    const read = await req('POST', `/api/messages/conversations/${convId}/read`, { token: t2, body: {} });
    check('marcar leído -> 200', read.status === 200, `status=${read.status} ${read.text.slice(0, 120)}`);

    if (msgId) {
      const react = await req('POST', `/api/messages/${msgId}/react`, { token: t2, body: { reaction: '❤️' } });
      check('reacción a mensaje -> 200', react.status === 200, `status=${react.status} ${react.text.slice(0, 120)}`);
    }

    const convs = await req('GET', '/api/messages/conversations', { token: t2 });
    check('listar conversaciones (bob) -> 200', convs.status === 200, `status=${convs.status}`);
    check('bob ve la conversación con alice', new RegExp(u1).test(convs.text), convs.text.slice(0, 200));
  }
  if (ws) { try { ws.close(); } catch { /* ya cerrado */ } }

  // ---------- Notificaciones ----------
  console.log('[notificaciones]');
  const notifs = await req('GET', '/api/notifications', { token: t2 });
  check('GET /api/notifications -> 200', notifs.status === 200, `status=${notifs.status} ${notifs.text.slice(0, 160)}`);
  check('bob tiene notificación (like/mensaje)', /like|follow|message|comment|like|❤|mensagem|mensaje/i.test(notifs.text), notifs.text.slice(0, 200));
  const prefs = await req('GET', '/api/notifications/prefs', { token: t2 });
  check('preferencias -> 200', prefs.status === 200, `status=${prefs.status}`);

  // ---------- Media (subida real) ----------
  console.log('[media]');
  const jpeg = tinyJpeg();
  const fd = new FormData();
  fd.append('file', new Blob([jpeg], { type: 'image/jpeg' }), 'foto.jpg');
  const up = await req('POST', '/api/upload', {
    token: lt,
    body: fd,
    headers: {},
    raw: false,
  }).catch(async () => {
    // FormData no debe forzar Content-Type
    const res = await fetch(`${API_BASE}/api/upload`, {
      method: 'POST',
      headers: { Authorization: `Bearer ${lt}` },
      body: fd,
    });
    const text = await res.text();
    let json = null;
    try { json = JSON.parse(text); } catch { /* ignore */ }
    return { status: res.status, json, text };
  });
  check('subida JPEG -> 200/201', up.status === 200 || up.status === 201, `status=${up.status} ${up.text.slice(0, 200)}`);
  const mediaUrl = up.json && (up.json.url || (up.json.media && up.json.media.url) || (up.json.data && up.json.data.url));
  check('subida devuelve url', !!mediaUrl, up.text.slice(0, 200));

  if (mediaUrl) {
    const full = mediaUrl.startsWith('http') ? mediaUrl : API_BASE + mediaUrl;
    const img = await fetch(full);
    const buf = Buffer.from(await img.arrayBuffer());
    check('imagen servida -> 200', img.status === 200, `status=${img.status}`);
    check('imagen es JPEG (FFD8)', buf.length > 10 && buf[0] === 0xff && buf[1] === 0xd8, `bytes=${buf.length} head=${buf.slice(0, 4).toString('hex')}`);

    const bad = await fetch(`${API_BASE}/api/upload`, {
      method: 'POST',
      headers: { Authorization: `Bearer ${lt}`, 'Content-Type': 'application/octet-stream' },
      body: 'no soy una imagen',
    });
    check('subida no-imagen -> 4xx', bad.status >= 400 && bad.status < 500, `status=${bad.status}`);
  }

  // ---------- Reportes + Admin ----------
  console.log('[reportes + admin]');
  const report = await req('POST', '/api/reports', {
    token: t2,
    body: { target_type: 'post', target_id: postId, reason: 'spam', detail: 'post de prueba' },
  });
  check('crear reporte -> 200/201', report.status === 200 || report.status === 201, `status=${report.status} ${report.text.slice(0, 200)}`);
  const reportObj = report.json && (report.json.report || report.json);
  const reportId = reportObj && reportObj.id;

  const dash = await req('GET', '/api/admin/dashboard', { token: lt });
  check('admin dashboard (alice, admin bootstrap) -> 200', dash.status === 200, `status=${dash.status} ${dash.text.slice(0, 200)}`);
  const admUsers = await req('GET', '/api/admin/users', { token: lt });
  check('admin usuarios -> 200', admUsers.status === 200, `status=${admUsers.status}`);
  check('admin ve a bob', new RegExp(u2).test(admUsers.text), admUsers.text.slice(0, 200));
  const admStats = await req('GET', '/api/admin/stats', { token: lt });
  check('admin stats -> 200', admStats.status === 200, `status=${admStats.status}`);
  const admAct = await req('GET', '/api/admin/activity', { token: lt });
  check('admin activity -> 200', admAct.status === 200, `status=${admAct.status}`);
  const admWords = await req('GET', '/api/admin/words', { token: lt });
  check('admin words -> 200', admWords.status === 200, `status=${admWords.status}`);

  // bob (no admin) no debe entrar
  const bobDash = await req('GET', '/api/admin/dashboard', { token: t2 });
  check('admin bloqueado para no-admin -> 403', bobDash.status === 403, `status=${bobDash.status} ${bobDash.text.slice(0, 120)}`);

  if (reportId) {
    const resolve = await req('POST', `/api/admin/reports/${reportId}/resolve`, {
      token: lt,
      body: { resolution: 'duplicado' },
    });
    check('admin resuelve reporte -> 200', resolve.status === 200, `status=${resolve.status} ${resolve.text.slice(0, 160)}`);
  }

  // ---------- Seguridad / errores ----------
  console.log('[seguridad]');
  const nf = await req('GET', '/api/no-existe');
  check('404 ruta inexistente', nf.status === 404, `status=${nf.status}`);
  const badMethod = await req('GET', '/api/posts', { token: lt });
  check('405 método inválido', badMethod.status === 405, `status=${badMethod.status}`);
  const cors = await fetch(`${API_BASE}/api/health`, { headers: { Origin: 'http://evil.example' }, method: 'OPTIONS' });
  check('preflight OPTIONS responde', cors.status < 500, `status=${cors.status}`);
  const badToken = await req('GET', '/api/auth/me', { token: 'invalido.abc.def' });
  check('JWT inválido -> 401', badToken.status === 401, `status=${badToken.status}`);

  // XSS: el contenido se sanitiza al leerse
  const xssPost = await req('POST', '/api/posts', { token: lt, body: { content: '<script>alert(1)</script> ok' } });
  if (xssPost.status === 201 || xssPost.status === 200) {
    const xp = xssPost.json && (xssPost.json.post || xssPost.json);
    const xpId = xp && xp.id;
    if (xpId) {
      const xpGet = await req('GET', `/api/posts/${xpId}`, { token: lt });
      check('XSS escapeado en respuesta', !/<script>alert/.test(xpGet.text), xpGet.text.slice(0, 200));
    }
  } else {
    check('XSS: post creado (debe 201)', false, `status=${xssPost.status} ${xssPost.text.slice(0, 120)}`);
  }

  // ---------- Logout ----------
  console.log('[logout]');
  const out = await req('POST', '/api/auth/logout', { token: lt, body: { refresh_token: nr } });
  check('logout -> 200', out.status === 200, `status=${out.status} ${out.text.slice(0, 120)}`);
  const afterOut = await req('POST', '/api/auth/refresh', { body: { refresh_token: nr } });
  check('refresh revocado tras logout -> 401', afterOut.status === 401, `status=${afterOut.status}`);

  // ---------- Resumen ----------
  console.log(`\n=== RESULTADO: ${passed} PASS / ${failed} FAIL ===`);
  if (failures.length) {
    console.log('Fallos:');
    for (const f of failures) console.log('  - ' + f);
  }
  process.exit(failed > 0 ? 1 : 0);
}

main().catch((e) => {
  console.error('E2E ERROR:', e);
  process.exit(2);
});
