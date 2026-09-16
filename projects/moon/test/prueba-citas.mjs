// Moon — Prueba real de «responder citando» en el chat
// Uso: API=http://127.0.0.1:3012 node test/prueba-citas.mjs
const API = (process.env.API || 'http://127.0.0.1:3000').replace(/\/$/, '');
let ok = 0, mal = 0;
const comprobar = (nombre, cond, extra = '') => {
  if (cond) { ok++; console.log(`  ok    ${nombre}`); }
  else { mal++; console.log(`  FALLO ${nombre} ${extra}`); }
};
async function pedir(metodo, ruta, { cuerpo, token } = {}) {
  const cabeceras = { 'content-type': 'application/json' };
  if (token) cabeceras.Authorization = `Bearer ${token}`;
  const r = await fetch(API + ruta, { method: metodo, headers: cabeceras, body: cuerpo ? JSON.stringify(cuerpo) : undefined });
  let d = null; try { d = await r.json(); } catch { d = null; }
  return { estado: r.status, d };
}
const sello = Date.now().toString().slice(-7);
const A = `cita${sello}a`, B = `cita${sello}b`, C = `cita${sello}c`;
const reg = async (u) => (await pedir('POST', '/api/auth/register', { cuerpo: { username: u, email: `${u}@moon.test`, password: 'ClaveSegura123' } })).d;
const a1 = await reg(A), b1 = await reg(B), c1 = await reg(C);
const tA = a1.access_token, tB = b1.access_token, tC = c1.access_token;
comprobar('tres cuentas de prueba creadas', !!tA && !!tB && !!tC);

const conv = await pedir('POST', '/api/messages/conversations', { cuerpo: { user_id: b1.user.id }, token: tA });
const cid = conv.d?.conversation_id || conv.d?.id;
comprobar('conversación creada entre las dos', !!cid, JSON.stringify(conv.d));

const m1 = await pedir('POST', `/api/messages/conversations/${cid}/messages`, { cuerpo: { content: '¿Revisaste el diseño nuevo?' }, token: tA });
comprobar('primer mensaje enviado', m1.estado === 200 || m1.estado === 201, String(m1.estado));
comprobar('sin cita, reply_to viene vacío', m1.d?.reply_to === null);

const m2 = await pedir('POST', `/api/messages/conversations/${cid}/messages`, { cuerpo: { content: 'Sí, quedó mucho mejor.', reply_to_id: m1.d.id }, token: tB });
comprobar('respuesta citando enviada', m2.estado === 200 || m2.estado === 201, JSON.stringify(m2.d));
comprobar('la cita trae autor y texto', m2.d?.reply_to?.content === '¿Revisaste el diseño nuevo?' && !!m2.d?.reply_to?.autor, JSON.stringify(m2.d?.reply_to));
comprobar('la cita apunta al mensaje correcto', Number(m2.d?.reply_to?.id) === Number(m1.d.id));

const hilo = await pedir('GET', `/api/messages/conversations/${cid}`, { token: tA });
const ultimo = hilo.d?.messages?.find((m) => m.id === m2.d.id);
comprobar('al abrir el hilo, la cita sigue ahí', ultimo?.reply_to?.content === '¿Revisaste el diseño nuevo?', JSON.stringify(ultimo?.reply_to));

// Conversación distinta (B con C) para probar que no se puede citar de fuera
const c2 = await pedir('POST', '/api/messages/conversations', { cuerpo: { user_id: c1.user.id }, token: tB });
const cid2 = c2.d?.conversation_id || c2.d?.id;
comprobar('segunda conversación aparte', Number(cid2) !== Number(cid), `${cid2} vs ${cid}`);
const fuera = await pedir('POST', `/api/messages/conversations/${cid2}/messages`, { cuerpo: { content: 'Esto no debería', reply_to_id: m1.d.id }, token: tB });
comprobar('no se puede citar un mensaje de otra conversación', fuera.estado === 400, `estado ${fuera.estado}`);
const noExiste = await pedir('POST', `/api/messages/conversations/${cid}/messages`, { cuerpo: { content: 'Ni este', reply_to_id: 99999999 }, token: tB });
comprobar('no se puede citar un mensaje inexistente', noExiste.estado === 400, `estado ${noExiste.estado}`);
const vacio = await pedir('POST', `/api/messages/conversations/${cid}/messages`, { cuerpo: { content: '   ' }, token: tB });
comprobar('sigue sin dejarse enviar un mensaje vacío', vacio.estado === 400, `estado ${vacio.estado}`);

console.log(`\n${mal === 0 ? 'TODO CORRECTO' : 'HAY FALLOS'} — ${ok} bien, ${mal} mal`);
process.exit(mal === 0 ? 0 : 1);
