// Moon — Prueba real de la tanda 4 (perfil completo)
// Uso: API=http://127.0.0.1:3012 node test/prueba-tanda4.mjs
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
const nombres = [`t4a${sello}`, `t4b${sello}`, `t4c${sello}`];
const reg = async (u) => (await pedir('POST', '/api/auth/register', { cuerpo: { username: u, email: `${u}@moon.test`, password: 'ClaveSegura123' } })).d;
const [a, b, c] = [await reg(nombres[0]), await reg(nombres[1]), await reg(nombres[2])];
const tA = a.access_token, tB = b.access_token, tC = c.access_token;
comprobar('tres cuentas de prueba', !!tA && !!tB && !!tC);

// ---------- 1) Listas de seguidores y seguidos ----------
await pedir('POST', `/api/users/${b.user.id}/follow`, { token: tA });
await pedir('POST', `/api/users/${b.user.id}/follow`, { token: tC });
const seguidores = await pedir('GET', `/api/users/${b.user.id}/followers`, { token: tA });
comprobar('la lista de seguidores trae a los dos', seguidores.estado === 200 && seguidores.d?.total === 2 && seguidores.d.items.length === 2, JSON.stringify([seguidores.estado, seguidores.d?.total]));
comprobar('cada fila dice si ya le sigo', seguidores.d.items.some((u) => u.username === nombres[0] && u.le_sigo === false), JSON.stringify(seguidores.d.items.map((u) => [u.username, u.le_sigo])));
const siguiendo = await pedir('GET', `/api/users/${a.user.id}/following`, { token: tC });
comprobar('la lista de seguidos también', siguiendo.d?.total === 1 && siguiendo.d.items[0].username === nombres[1], JSON.stringify([siguiendo.d?.total, siguiendo.d?.items?.[0]?.username]));
const d4 = await reg(`t4d${sello}`);
const tD = d4.access_token;
const listaVacia = await pedir('GET', `/api/users/${d4.user.id}/following`, { token: tD });
const listaVacia2 = await pedir('GET', `/api/users/${d4.user.id}/followers`, { token: tD });
comprobar('quien no sigue a nadie devuelve lista vacía',
  listaVacia.d?.total === 0 && listaVacia.d.items.length === 0 && listaVacia2.d?.total === 0,
  JSON.stringify([listaVacia.d?.total, listaVacia2.d?.total]));

// ---------- 2) Cuenta privada: solicitudes ----------
await pedir('PATCH', '/api/auth/privacy', { cuerpo: { is_private: true }, token: tC });
const pedirSeguir = await pedir('POST', `/api/users/${c.user.id}/follow`, { token: tB });
comprobar('seguir una cuenta privada pide permiso', pedirSeguir.d?.solicitado === true && pedirSeguir.d?.is_following === false, JSON.stringify(pedirSeguir.d));
const perfilPrivado = await pedir('GET', `/api/users/${c.user.id}`, { token: tB });
comprobar('el perfil privado lo dice', perfilPrivado.d?.is_private === true && perfilPrivado.d?.solicitud_enviada === true, JSON.stringify([perfilPrivado.d?.is_private, perfilPrivado.d?.solicitud_enviada]));
const pedirOtraVez = await pedir('POST', `/api/users/${c.user.id}/follow`, { token: tB });
comprobar('no se puede pedir dos veces', pedirOtraVez.d?.solicitado === true, JSON.stringify(pedirOtraVez.d));
const solicitudes = await pedir('GET', '/api/me/solicitudes', { token: tC });
comprobar('el dueño ve la solicitud', solicitudes.d?.solicitudes?.length === 1 && solicitudes.d.solicitudes[0].username === nombres[1], JSON.stringify(solicitudes.d?.solicitudes));
const rechazar = await pedir('POST', `/api/me/solicitudes/${solicitudes.d.solicitudes[0].solicitud_id}`, { cuerpo: { aceptar: false }, token: tC });
comprobar('se puede rechazar', rechazar.d?.aceptada === false, JSON.stringify(rechazar.d));
const sigueFuera = await pedir('GET', `/api/users/${c.user.id}/followers`, { token: tC });
comprobar('quien fue rechazado no está dentro', sigueFuera.d?.total === 0, JSON.stringify(sigueFuera.d?.total));
await pedir('POST', `/api/users/${c.user.id}/follow`, { token: tB });
const deNuevo = await pedir('GET', '/api/me/solicitudes', { token: tC });
const aceptar = await pedir('POST', `/api/me/solicitudes/${deNuevo.d.solicitudes[0].solicitud_id}`, { cuerpo: { aceptar: true }, token: tC });
comprobar('se puede aceptar', aceptar.d?.aceptada === true, JSON.stringify(aceptar.d));
const yaDentro = await pedir('GET', `/api/users/${c.user.id}/followers`, { token: tC });
comprobar('al aceptar aparece como seguidor', yaDentro.d?.total === 1 && yaDentro.d.items[0].username === nombres[1], JSON.stringify(yaDentro.d?.items?.map((u) => u.username)));
const borrarSolicitud = await pedir('POST', `/api/users/${c.user.id}/follow`, { token: tA });
const listaSolicitudes = await pedir('GET', '/api/me/solicitudes', { token: tC });
const idPendiente = listaSolicitudes.d?.solicitudes?.[0]?.solicitud_id;
comprobar('queda una solicitud sin aceptar', !!idPendiente, JSON.stringify(listaSolicitudes.d?.solicitudes));
void borrarSolicitud;
const quitar = await pedir('DELETE', `/api/me/solicitudes/${idPendiente}`, { token: tC });
comprobar('una solicitud se puede retirar sin aceptarla', quitar.estado === 200, String(quitar.estado));

// ---------- 3) Mis me gusta y mis comentarios ----------
const p1 = await pedir('POST', '/api/posts', { cuerpo: { content: `Publicación de ${sello} para probar likes` }, token: tA });
const p2 = await pedir('POST', '/api/posts', { cuerpo: { content: `Otra de ${sello} para comentar` }, token: tA });
await pedir('POST', `/api/posts/${p1.d.id}/like`, { cuerpo: {}, token: tB });
await pedir('POST', `/api/posts/${p1.d.id}/comments`, { cuerpo: { content: `Buen aporte ${sello}` }, token: tB });
const misLikes = await pedir('GET', '/api/me/likes', { token: tB });
if (Array.isArray(misLikes.d)) misLikes.d = { items: misLikes.d, total: misLikes.d.length };
comprobar('mis me gusta traen la publicación', misLikes.d?.total >= 1 && (misLikes.d.items || []).some((p) => p.id === p1.d.id), JSON.stringify([misLikes.d?.total, (misLikes.d?.items || []).map((p) => p.id)]));
const misComentarios = await pedir('GET', '/api/me/comments', { token: tB });
const mio = (misComentarios.d?.items || []).find((x) => x.post_id === p1.d.id);
comprobar('mis comentarios traen el texto y de quién era', !!mio && mio.content.includes(sello) && mio.post_autor.length > 0, JSON.stringify(mio));
comprobar('el conteo de comentarios cuadra', misComentarios.d?.total >= 1, JSON.stringify(misComentarios.d?.total));
void p2;

// ---------- 4) Historial de búsqueda ----------
await pedir('GET', `/api/search?q=${nombres[2]}&type=users`, { token: tA });
await new Promise((r) => setTimeout(r, 300));
const historial = await pedir('GET', '/api/me/busquedas', { token: tA });
comprobar('al buscar queda en el historial', (historial.d?.busquedas || []).some((b) => b.termino === nombres[2]), JSON.stringify(historial.d?.busquedas));
await pedir('POST', '/api/me/busquedas', { cuerpo: { termino: 'fotografía nocturna', tipo: 'tags' }, token: tA });
const historial2 = await pedir('GET', '/api/me/busquedas', { token: tA });
comprobar('se puede guardar un término a mano', (historial2.d?.busquedas || [])[0]?.termino === 'fotografía nocturna', JSON.stringify(historial2.d?.busquedas?.[0]));
const repetido = await pedir('POST', '/api/me/busquedas', { cuerpo: { termino: 'fotografía nocturna' }, token: tA });
const historial3 = await pedir('GET', '/api/me/busquedas', { token: tA });
comprobar('repetir un término no lo duplica', historial3.d.busquedas.filter((b) => b.termino === 'fotografía nocturna').length === 1, JSON.stringify(historial3.d.busquedas.length));
void repetido;
const borrarUno = await pedir('DELETE', `/api/me/busquedas/${historial3.d.busquedas[0].id}`, { token: tA });
comprobar('se puede borrar una búsqueda', borrarUno.estado === 200, String(borrarUno.estado));
const borrarTodo = await pedir('DELETE', '/api/me/busquedas', { token: tA });
const historial4 = await pedir('GET', '/api/me/busquedas', { token: tA });
comprobar('se puede vaciar el historial', borrarTodo.estado === 200 && historial4.d.busquedas.length === 0, JSON.stringify(historial4.d.busquedas.length));

// ---------- 5) Seguir etiquetas ----------
const seguirTag = await pedir('POST', `/api/hashtags/diseno${sello}/follow`, { token: tA });
comprobar('se puede seguir una etiqueta', seguirTag.d?.siguiendo === true, JSON.stringify(seguirTag.d));
const tw = await pedir('POST', '/api/posts', { cuerpo: { content: `Probando etiquetas #diseno${sello} y más` }, token: tB });
void tw;
const misTags = await pedir('GET', '/api/me/hashtags', { token: tA });
comprobar('la etiqueta queda en mi lista', (misTags.d?.hashtags || []).some((h) => h.tag === `diseno${sello}`), JSON.stringify(misTags.d?.hashtags));
const feedTags = await pedir('GET', '/api/feed/etiquetas', { token: tA });
comprobar('el rincón de etiquetas trae lo de esa etiqueta', (feedTags.d?.items || []).some((p) => p.content.includes(`diseno${sello}`)), JSON.stringify((feedTags.d?.items || []).map((p) => p.content?.slice(0, 30))));
const dejarTag = await pedir('DELETE', `/api/hashtags/diseno${sello}/follow`, { token: tA });
comprobar('se puede dejar de seguir', dejarTag.d?.siguiendo === false, JSON.stringify(dejarTag.d));

// ---------- 6) Ajustes: idioma, oscuro por horario, ahorro, PIN ----------
const ajustes = await pedir('GET', '/api/me/ajustes', { token: tA });
comprobar('los ajustes vienen con valores por defecto', ajustes.estado === 200 && ajustes.d?.idioma === 'es' && ajustes.d?.tema_auto === false && ajustes.d?.tiene_pin === false, JSON.stringify(ajustes.d));
const cambio = await pedir('PATCH', '/api/me/ajustes', { cuerpo: { idioma: 'en', tema_auto: true, tema_desde: '21:30', tema_hasta: '06:45', ahorro_datos: true, avisos_tipos: { likes: false, follows: true } }, token: tA });
comprobar('se guardan idioma, horario y ahorro', cambio.d?.idioma === 'en' && cambio.d?.tema_auto === true && cambio.d?.tema_desde === '21:30' && cambio.d?.ahorro_datos === true, JSON.stringify(cambio.d));
comprobar('los avisos filtrables se guardan', cambio.d?.avisos_tipos?.likes === false && cambio.d?.avisos_tipos?.follows === true, JSON.stringify(cambio.d?.avisos_tipos));
const horaMala = await pedir('PATCH', '/api/me/ajustes', { cuerpo: { tema_desde: '25:99' }, token: tA });
comprobar('una hora imposible no se guarda', horaMala.d?.tema_desde === '21:30', JSON.stringify(horaMala.d?.tema_desde));
const idiomaMalo = await pedir('PATCH', '/api/me/ajustes', { cuerpo: { idioma: 'klingon' }, token: tA });
comprobar('un idioma que no existe se ignora', idiomaMalo.d?.idioma === 'en', JSON.stringify(idiomaMalo.d?.idioma));
const pinCorto = await pedir('POST', '/api/me/ajustes/pin', { cuerpo: { pin: '12' }, token: tA });
comprobar('un PIN corto se rechaza', pinCorto.estado === 400, String(pinCorto.estado));
const pin = await pedir('POST', '/api/me/ajustes/pin', { cuerpo: { pin: '4821' }, token: tA });
comprobar('se pone un PIN', pin.d?.tiene_pin === true && pin.d?.bloqueo_activo === true, JSON.stringify(pin.d));
const pinMal = await pedir('POST', '/api/me/ajustes/pin/verificar', { cuerpo: { pin: '0000' }, token: tA });
comprobar('un PIN equivocado no pasa', pinMal.estado === 403, String(pinMal.estado));
const pinBien = await pedir('POST', '/api/me/ajustes/pin/verificar', { cuerpo: { pin: '4821' }, token: tA });
comprobar('el PIN correcto sí pasa', pinBien.d?.correcto === true, JSON.stringify(pinBien.d));
const cambiarPin = await pedir('POST', '/api/me/ajustes/pin', { cuerpo: { pin: '9999', pin_actual: '0000' }, token: tA });
comprobar('cambiar el PIN pide el anterior', cambiarPin.estado === 403, String(cambiarPin.estado));
const pinFuera = await pedir('POST', '/api/me/ajustes/pin', { cuerpo: { pin: '' }, token: tA });
comprobar('el PIN se puede quitar', pinFuera.d?.tiene_pin === false, JSON.stringify(pinFuera.d));

// ---------- 7) Exportar mis datos ----------
await pedir('POST', `/api/users/${b.user.id}/follow`, { token: tA });
await pedir('POST', '/api/me/busquedas', { cuerpo: { termino: 'mis datos' }, token: tA });
const copia = await pedir('GET', '/api/me/exportar', { token: tA });
comprobar('la copia trae mi perfil', copia.d?.perfil?.username === nombres[0], JSON.stringify(copia.d?.perfil?.username));
comprobar('la copia trae mis publicaciones y comentarios', Array.isArray(copia.d?.publicaciones) && Array.isArray(copia.d?.comentarios), JSON.stringify([copia.d?.publicaciones?.length, copia.d?.comentarios?.length]));
comprobar('la copia trae a quién sigo y mis etiquetas', Array.isArray(copia.d?.siguiendo) && Array.isArray(copia.d?.etiquetas), JSON.stringify([copia.d?.siguiendo, copia.d?.etiquetas]));
comprobar('la copia no lleva el PIN ni hashes', !JSON.stringify(copia.d).includes('pin_hash'), 'revisar');
const copyB = await pedir('GET', '/api/me/exportar', { token: tB });
comprobar('cada quien exporta solo lo suyo', copyB.d?.perfil?.username === nombres[1] && !JSON.stringify(copyB.d).includes(nombres[0] + '@moon.test'), JSON.stringify(copyB.d?.perfil?.username));

console.log(`\n${mal === 0 ? 'TODO CORRECTO' : 'HAY FALLOS'} — ${ok} bien, ${mal} mal`);
process.exit(mal === 0 ? 0 : 1);
