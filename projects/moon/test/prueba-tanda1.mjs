// Prueba real de la tanda 1 contra el servidor :3012
const API = 'http://127.0.0.1:3012';
let ok = 0, mal = 0;
const comprobar = (nombre, cond, extra = '') => {
  if (cond) { ok++; console.log(`  ✓ ${nombre}`); }
  else { mal++; console.log(`  ✗ ${nombre} ${extra}`); }
};
async function pedir(metodo, ruta, { cuerpo, token } = {}) {
  const cabeceras = { 'content-type': 'application/json' };
  if (token) cabeceras.Authorization = `Bearer ${token}`;
  const r = await fetch(API + ruta, { method: metodo, headers: cabeceras, body: cuerpo ? JSON.stringify(cuerpo) : undefined });
  let d = null; try { d = await r.json(); } catch { d = null; }
  return { estado: r.status, d };
}
const sello = Date.now().toString().slice(-6);
const A = `t1a${sello}`, B = `t1b${sello}`;
const reg = async (u) => (await pedir('POST', '/api/auth/register', { cuerpo: { username: u, email: `${u}@moon.test`, password: 'ClaveSegura123' } })).d;
const a = await reg(A), b = await reg(B);
const tA = a.access_token, tB = b.access_token;
comprobar('cuentas de prueba', !!tA && !!tB);

const pub = await pedir('POST', '/api/posts', { cuerpo: { content: `Hola @${B}, mira esto #prueba` }, token: tA });
comprobar('publicación creada', !!pub.d?.id, JSON.stringify(pub.d).slice(0, 120));
const pid = pub.d.id;
comprobar('la publicación trae el resumen de reacciones vacío', pub.d?.reacciones?.total === 0 && pub.d?.reacciones?.mi === null, JSON.stringify(pub.d?.reacciones));

// 1) Reacciones variadas
const r1 = await pedir('POST', `/api/posts/${pid}/react`, { cuerpo: { tipo: 'risa' }, token: tB });
comprobar('reaccionar con risa', r1.estado === 200 && r1.d?.reacciones?.mi === 'risa', `${r1.estado} ${JSON.stringify(r1.d?.reacciones)}`);
comprobar('el contador sube a 1', r1.d?.reacciones?.total === 1 && r1.d?.likes_count === 1, JSON.stringify({ t: r1.d?.reacciones?.total, l: r1.d?.likes_count }));
const r2 = await pedir('POST', `/api/posts/${pid}/react`, { cuerpo: { tipo: 'sorpresa' }, token: tB });
comprobar('cambiar de reacción no duplica', r2.d?.reacciones?.total === 1 && r2.d?.reacciones?.mi === 'sorpresa' && r2.d?.likes_count === 1, JSON.stringify({ t: r2.d?.reacciones?.total, mi: r2.d?.reacciones?.mi, l: r2.d?.likes_count }));
comprobar('la reacción vieja desaparece del conteo', !r2.d?.reacciones?.conteo?.risa, JSON.stringify(r2.d?.reacciones?.conteo));
const r3 = await pedir('POST', `/api/posts/${pid}/react`, { cuerpo: { tipo: 'inventada' }, token: tB });
comprobar('una reacción que no existe se rechaza', r3.estado === 400, String(r3.estado));
const r4 = await pedir('DELETE', `/api/posts/${pid}/react`, { token: tB });
comprobar('quitar la reacción baja el contador', r4.d?.reacciones?.total === 0 && r4.d?.likes_count === 0, JSON.stringify({ t: r4.d?.reacciones?.total, l: r4.d?.likes_count }));
const r5 = await pedir('GET', `/api/posts/${pid}/reacciones`, { token: tA });
comprobar('la lista de quién reaccionó responde', Array.isArray(r5.d?.personas) && r5.d?.total === 0, JSON.stringify(r5.d).slice(0, 100));

// 2) Fijar publicación
const p1 = await pedir('POST', `/api/posts/${pid}/pin`, { token: tA });
comprobar('fijar mi publicación', p1.d?.pinned === true, JSON.stringify(p1.d?.pinned));
const p2 = await pedir('POST', `/api/posts/${pid}/pin`, { cuerpo: {}, token: tB });
comprobar('no puedo fijar la de otro', p2.estado === 403, String(p2.estado));
const pub2 = await pedir('POST', '/api/posts', { cuerpo: { content: 'Segunda publicación' }, token: tA });
await pedir('POST', `/api/posts/${pub2.d.id}/pin`, { token: tA });
const perfilA = await pedir('GET', `/api/users/${a.user.id}/posts`, { token: tA });
comprobar('solo queda una fijada (la última)', perfilA.d?.items?.filter((x) => x.pinned).length === 1, JSON.stringify(perfilA.d?.items?.map((x) => x.pinned)));
comprobar('la fijada va primera en el perfil', perfilA.d?.items?.[0]?.pinned === true, JSON.stringify(perfilA.d?.items?.[0]?.id));

// 3) No me interesa
const verAntes = await pedir('GET', '/api/feed', { token: tB });
const estabaAntes = (verAntes.d?.items || []).some((x) => x.id === pid);
const ni = await pedir('POST', `/api/posts/${pid}/interesa`, { cuerpo: { no: true }, token: tB });
comprobar('marcar «no me interesa»', ni.estado === 200 && ni.d?.no_interesa === true, JSON.stringify(ni.d));
const verDespues = await pedir('GET', '/api/feed', { token: tB });
comprobar('desaparece del feed de quien lo marcó', estabaAntes && !(verDespues.d?.items || []).some((x) => x.id === pid), `antes:${estabaAntes}`);
const verOtro = await pedir('GET', '/api/feed', { token: tA });
comprobar('sigue visible para los demás', (verOtro.d?.items || []).some((x) => x.id === pid));
await pedir('POST', `/api/posts/${pid}/interesa`, { cuerpo: { no: false }, token: tB });
const verVuelta = await pedir('GET', '/api/feed', { token: tB });
comprobar('al deshacerlo vuelve a aparecer', (verVuelta.d?.items || []).some((x) => x.id === pid));

// 4) Menciones con @
const avisosB = await pedir('GET', '/api/notifications', { token: tB });
const menciones = (avisosB.d?.items || []).filter((x) => x.type === 'mention');
comprobar('la mención en la publicación avisa', menciones.length >= 1, JSON.stringify(menciones.slice(0, 1)));
const comMencion = await pedir('POST', `/api/posts/${pid}/comments`, { cuerpo: { content: `Hola @${A}, gracias` }, token: tB });
const avisosA = await pedir('GET', '/api/notifications', { token: tA });
const mencionesA = (avisosA.d?.items || []).filter((x) => x.type === 'mention');
comprobar('la mención en un comentario avisa', mencionesA.length >= 1, JSON.stringify(mencionesA.slice(0, 1)));

// 5) Comentarios: reacciones, fijado, orden y etiqueta de autor
// Un comentario de quien publicó (A) y otro de un tercero (B).
const comDeA = await pedir('POST', `/api/posts/${pid}/comments`, { cuerpo: { content: 'Comentario mío como autor' }, token: tA });
const comentarios = await pedir('GET', `/api/posts/${pid}/comments`, { token: tA });
const deA = comentarios.d?.find((x) => x.id === comDeA.d.id);
const deB = comentarios.d?.find((x) => x.username === B);
comprobar('el comentario trae reacciones vacías', deA?.reacciones?.total === 0, JSON.stringify(deA?.reacciones));
comprobar('el comentario de quien publicó viene marcado como Autor', deA?.es_autor === true, JSON.stringify({ es_autor: deA?.es_autor, u: deA?.username }));
comprobar('el de otra persona no lleva la etiqueta de Autor', deB?.es_autor === false, JSON.stringify({ es_autor: deB?.es_autor, u: deB?.username }));
const c1 = deA;
const cr = await pedir('POST', `/api/comments/${c1.id}/react`, { cuerpo: { tipo: 'me_encanta' }, token: tA });
comprobar('reaccionar al comentario', cr.d?.reacciones?.mi === 'me_encanta' && cr.d?.reacciones?.total === 1, JSON.stringify(cr.d?.reacciones));
const cp = await pedir('POST', `/api/comments/${c1.id}/pin`, { token: tA });
comprobar('fijar un comentario (soy el autor del post)', cp.d?.fijado === true, JSON.stringify(cp.d));
const cpAjeno = await pedir('POST', `/api/comments/${c1.id}/pin`, { token: tB });
comprobar('otro no puede fijar el comentario', cpAjeno.estado === 403, String(cpAjeno.estado));
const listaFijada = await pedir('GET', `/api/posts/${pid}/comments`, { token: tA });
comprobar('el fijado va primero', listaFijada.d?.[0]?.pinned === true, JSON.stringify(listaFijada.d?.map((x) => x.pinned)));
const porMejores = await pedir('GET', `/api/posts/${pid}/comments?orden=mejores`, { token: tA });
comprobar('el orden por mejores responde', Array.isArray(porMejores.d) && porMejores.d[0]?.pinned === true, JSON.stringify(porMejores.d?.length));

// 6) Encuesta: cierre y quién votó
const conEncuesta = await pedir('POST', '/api/posts', {
  cuerpo: { content: '¿Cuál diseño?', poll: { pregunta: '¿Cuál te gusta más?', opciones: ['Claro', 'Oscuro'], horas: 6 } },
  token: tA,
});
const eid = conEncuesta.d.id;
comprobar('la encuesta trae el cierre en palabras', !!conEncuesta.d?.poll?.cierra, JSON.stringify(conEncuesta.d?.poll));
await pedir('POST', `/api/posts/${eid}/vote`, { cuerpo: { opcion: 1 }, token: tB });
const votos = await pedir('GET', `/api/posts/${eid}/poll/votos`, { token: tA });
comprobar('quien creó la encuesta ve quién votó', votos.d?.opciones?.some((o) => o.personas?.length === 1), JSON.stringify(votos.d).slice(0, 160));
const votosAjenos = await pedir('GET', `/api/posts/${eid}/poll/votos`, { token: tB });
comprobar('otro no ve los votos', votosAjenos.estado === 403, String(votosAjenos.estado));

console.log(`\n${mal === 0 ? 'TODO CORRECTO' : 'HAY FALLOS'} — ${ok} bien, ${mal} mal`);
process.exit(mal === 0 ? 0 : 1);
