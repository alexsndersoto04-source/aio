// Moon — Prueba real de la tanda 2 (mensajería completa)
// Uso: API=http://127.0.0.1:3012 node test/prueba-tanda2.mjs
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
const sello = Date.now().toString().slice(-6);
const A = `t2a${sello}`, B = `t2b${sello}`, C = `t2c${sello}`;
const reg = async (u) => (await pedir('POST', '/api/auth/register', { cuerpo: { username: u, email: `${u}@moon.test`, password: 'ClaveSegura123' } })).d;
const a = await reg(A), b = await reg(B), c = await reg(C);
const tA = a.access_token, tB = b.access_token, tC = c.access_token;
comprobar('tres cuentas de prueba', !!tA && !!tB && !!tC);

const conv = await pedir('POST', '/api/messages/conversations', { cuerpo: { user_id: b.user.id }, token: tA });
const cid = conv.d.conversation_id;
const conv2 = await pedir('POST', '/api/messages/conversations', { cuerpo: { user_id: c.user.id }, token: tA });
const cid2 = conv2.d.conversation_id;
comprobar('dos conversaciones abiertas', !!cid && !!cid2 && cid !== cid2);

// ---------- 1) Editar un mensaje enviado ----------
const m1 = await pedir('POST', `/api/messages/conversations/${cid}/messages`, { cuerpo: { content: 'Hola, esto tiene un erro' }, token: tA });
comprobar('mensaje enviado sin ediciones', m1.d?.edited_at === null, JSON.stringify(m1.d?.edited_at));
const ed = await pedir('PATCH', `/api/messages/${m1.d.id}`, { cuerpo: { content: 'Hola, esto es lo correcto' }, token: tA });
comprobar('editar mi mensaje', ed.estado === 200 && ed.d?.content === 'Hola, esto es lo correcto', `${ed.estado} ${JSON.stringify(ed.d?.content)}`);
comprobar('queda marcado como editado', !!ed.d?.edited_at, JSON.stringify(ed.d?.edited_at));
const edAjeno = await pedir('PATCH', `/api/messages/${m1.d.id}`, { cuerpo: { content: 'No debería' }, token: tB });
comprobar('no puedo editar el mensaje de otro', edAjeno.estado === 403, String(edAjeno.estado));
const edVacio = await pedir('PATCH', `/api/messages/${m1.d.id}`, { cuerpo: { content: '   ' }, token: tA });
comprobar('no se puede dejar vacío', edVacio.estado === 400, String(edVacio.estado));
const hilo1 = await pedir('GET', `/api/messages/conversations/${cid}`, { token: tB });
comprobar('el otro ve el texto ya editado', hilo1.d?.messages?.find((m) => m.id === m1.d.id)?.content === 'Hola, esto es lo correcto');

// ---------- 2) Reenviar a otra conversación ----------
const re = await pedir('POST', `/api/messages/${m1.d.id}/forward`, { cuerpo: { conversation_id: cid2 }, token: tA });
comprobar('reenviar a otro chat', re.estado === 200 && re.d?.content === 'Hola, esto es lo correcto', `${re.estado}`);
comprobar('el reenviado vive en la otra conversación', Number(re.d?.conversation_id) === Number(cid2), JSON.stringify(re.d?.conversation_id));
const hilo2 = await pedir('GET', `/api/messages/conversations/${cid2}`, { token: tC });
comprobar('el destinatario lo recibe', (hilo2.d?.messages || []).some((m) => m.id === re.d.id));
const reMalo = await pedir('POST', `/api/messages/${m1.d.id}/forward`, { cuerpo: { conversation_id: 999999 }, token: tA });
comprobar('reenviar a un chat que no existe falla', reMalo.estado === 404, String(reMalo.estado));

// ---------- 3) Compartir una publicación al chat ----------
const pub = await pedir('POST', '/api/posts', { cuerpo: { content: 'Publicación para compartir en el chat' }, token: tA });
const comp = await pedir('POST', `/api/messages/conversations/${cid}/messages`, { cuerpo: { post_id: pub.d.id }, token: tA });
comprobar('compartir la publicación al chat', comp.estado === 200 && Number(comp.d?.post?.id) === Number(pub.d.id), `${comp.estado} ${JSON.stringify(comp.d?.post)}`);
comprobar('la tarjeta trae autor y texto', comp.d?.post?.autor === a.user.display_name || !!comp.d?.post?.autor, JSON.stringify(comp.d?.post));
const compMala = await pedir('POST', `/api/messages/conversations/${cid}/messages`, { cuerpo: { post_id: 99999999 }, token: tA });
comprobar('una publicación que no existe se rechaza', compMala.estado === 404, String(compMala.estado));

// ---------- 4) Fijar un mensaje ----------
const pin = await pedir('POST', `/api/messages/${m1.d.id}/pin`, { token: tA });
comprobar('fijar un mensaje', pin.estado === 200 && pin.d?.fijado === true, JSON.stringify(pin.d));
const hiloPin = await pedir('GET', `/api/messages/conversations/${cid}`, { token: tB });
comprobar('el fijado viaja en el hilo (lo ve el otro)', hiloPin.d?.pinned?.id === m1.d.id, JSON.stringify(hiloPin.d?.pinned?.id));
const pin2 = await pedir('POST', `/api/messages/${m1.d.id}/pin`, { token: tA });
comprobar('volver a pulsar lo suelta', pin2.d?.fijado === false, JSON.stringify(pin2.d));
const hiloSinPin = await pedir('GET', `/api/messages/conversations/${cid}`, { token: tB });
comprobar('el hilo queda sin fijado', hiloSinPin.d?.pinned === null, JSON.stringify(hiloSinPin.d?.pinned));
await pedir('POST', `/api/messages/${m1.d.id}/pin`, { token: tA });
const borradoPin = await pedir('DELETE', `/api/messages/${comp.d.id}`, { token: tA });
comprobar('se puede borrar un mensaje compartido', borradoPin.estado === 200, String(borradoPin.estado));

// ---------- 5) Marcar como no leída ----------
const nl = await pedir('POST', `/api/messages/conversations/${cid}/no-leida`, { cuerpo: { no: true }, token: tA });
comprobar('marcar como no leída', nl.estado === 200 && nl.d?.no_leida === true, JSON.stringify(nl.d));
const lista = await pedir('GET', '/api/messages/conversations', { token: tA });
const fila = (lista.d || []).find((x) => x.id === cid);
comprobar('la lista la trae marcada', fila?.no_leida === true, JSON.stringify(fila?.no_leida));
const leida = await pedir('POST', `/api/messages/conversations/${cid}/read`, { token: tA });
comprobar('al abrirla se limpia la marca', leida.estado === 200);
const lista2 = await pedir('GET', '/api/messages/conversations', { token: tA });
comprobar('la marca desaparece al leer', (lista2.d || []).find((x) => x.id === cid)?.no_leida === false, JSON.stringify((lista2.d || []).find((x) => x.id === cid)?.no_leida));

// ---------- 6) Detalle de «visto» ----------
const mB = await pedir('POST', `/api/messages/conversations/${cid}/messages`, { cuerpo: { content: '¿Me confirmas?' }, token: tA });
comprobar('mensaje nuevo sin leer todavía', mB.d?.read_at === null, JSON.stringify(mB.d?.read_at));
await pedir('POST', `/api/messages/conversations/${cid}/read`, { token: tB });
const hiloVisto = await pedir('GET', `/api/messages/conversations/${cid}`, { token: tA });
const visto = hiloVisto.d?.messages?.find((m) => m.id === mB.d.id);
comprobar('al leerlo, quien envió ve la hora exacta', !!visto?.read_at, JSON.stringify(visto?.read_at));
comprobar('el estado pasa a «read»', visto?.status === 'read', JSON.stringify(visto?.status));

// ---------- 7) Borrar la conversación (solo para mí) ----------
const borrar = await pedir('DELETE', `/api/messages/conversations/${cid2}`, { token: tA });
comprobar('borrar la conversación', borrar.estado === 200, String(borrar.estado));
const listaA = await pedir('GET', '/api/messages/conversations', { token: tA });
comprobar('desaparece de mi lista', !(listaA.d || []).some((x) => x.id === cid2), JSON.stringify((listaA.d || []).map((x) => x.id)));
const listaC = await pedir('GET', '/api/messages/conversations', { token: tC });
comprobar('la otra persona la sigue viendo', (listaC.d || []).some((x) => x.id === cid2), JSON.stringify((listaC.d || []).map((x) => x.id)));
const hiloC = await pedir('GET', `/api/messages/conversations/${cid2}`, { token: tC });
comprobar('sus mensajes siguen ahí', (hiloC.d?.messages || []).length > 0, JSON.stringify((hiloC.d?.messages || []).length));
const nuevoMsg = await pedir('POST', `/api/messages/conversations/${cid2}/messages`, { cuerpo: { content: 'Te escribo otra vez' }, token: tC });
const listaVuelve = await pedir('GET', '/api/messages/conversations', { token: tA });
comprobar('si me escriben, la conversación vuelve', (listaVuelve.d || []).some((x) => x.id === cid2), JSON.stringify(nuevoMsg.estado));

console.log(`\n${mal === 0 ? 'TODO CORRECTO' : 'HAY FALLOS'} — ${ok} bien, ${mal} mal`);
process.exit(mal === 0 ? 0 : 1);
