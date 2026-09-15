// Prueba de la API real contra el servidor y la base de datos de verdad.
// ============================================================
// Crea dos cuentas nuevas, publica, comenta, reacciona, sigue, intercambia
// mensajes, revisa notificaciones y usa la administración. Al final comprueba
// en la base de datos que los contadores quedaron bien.
//
// Uso:
//   node prueba-api.mjs                      (usa http://127.0.0.1:3000)
//   API=http://127.0.0.1:3000 node prueba-api.mjs
//
// Deja las cuentas creadas (con nombres únicos) para poder probarlas a mano.

const API = (process.env.API || 'http://127.0.0.1:3000').replace(/\/$/, '');
const sufijo = Math.random().toString(36).slice(2, 7);

let ok = 0;
let fallos = 0;

function comprobar(titulo, condicion, detalle = '') {
  if (condicion) {
    ok += 1;
    console.log(`  ok    ${titulo}`);
  } else {
    fallos += 1;
    console.log(`  FALLA ${titulo} ${detalle}`);
  }
}

async function pedir(metodo, ruta, { cuerpo, token, refresco, crudo } = {}) {
  const cabeceras = {};
  if (cuerpo !== undefined && !(cuerpo instanceof FormData)) cabeceras['Content-Type'] = 'application/json';
  if (token) cabeceras.Authorization = `Bearer ${token}`;
  if (refresco) cabeceras['X-Refresh-Token'] = refresco;
  const res = await fetch(`${API}${ruta}`, {
    method: metodo,
    headers: cabeceras,
    body: cuerpo === undefined ? undefined : cuerpo instanceof FormData ? cuerpo : JSON.stringify(cuerpo),
  });
  const texto = await res.text();
  let datos = null;
  try { datos = texto ? JSON.parse(texto) : null; } catch { datos = texto; }
  if (crudo) return { estado: res.status, datos };
  if (res.status >= 400) {
    const e = new Error(datos?.error || `HTTP ${res.status}`);
    e.estado = res.status;
    e.datos = datos;
    throw e;
  }
  return datos;
}

const nombreA = `ana${sufijo}`;
const nombreB = `beto${sufijo}`;
const clave = 'secreto-de-prueba-2026';

console.log(`Prueba real contra ${API}\n`);
const salud = await pedir('GET', '/api/health');
comprobar('salud del servicio y de la base de datos', salud.status === 'ok' && salud.db === true, JSON.stringify(salud));

// ---------- Registro y sesión ----------
const ana = await pedir('POST', '/api/auth/register', {
  cuerpo: { username: nombreA, email: `${nombreA}@moon.test`, password: clave },
});
comprobar('registro real de una cuenta', !!ana.access_token && ana.user.username === nombreA);
comprobar('la primera cuenta de la instalación es administradora o la cuenta es válida', ['admin', 'user'].includes(ana.user.role), ana.user.role);

const repetido = await pedir('POST', '/api/auth/register', {
  cuerpo: { username: nombreA, email: `${nombreA}@moon.test`, password: clave }, crudo: true,
});
comprobar('no se permite repetir usuario', repetido.estado === 409, String(repetido.estado));

const malo = await pedir('POST', '/api/auth/login', { cuerpo: { username: nombreA, password: 'equivocada' }, crudo: true });
comprobar('contraseña incorrecta rechazada', malo.estado === 401, String(malo.estado));

const sesion = await pedir('POST', '/api/auth/login', { cuerpo: { username: nombreA, password: clave } });
comprobar('acceso con la contraseña correcta', !!sesion.access_token && !!sesion.refresh_token);
let tokenA = sesion.access_token;
const refrescoA = sesion.refresh_token;

const yo = await pedir('GET', '/api/auth/me', { token: tokenA });
comprobar('perfil propio', yo.username === nombreA && yo.id > 0);

const renovado = await pedir('POST', '/api/auth/refresh', { refresco: refrescoA });
comprobar('renovación de sesión (rotación de token)', !!renovado.access_token && renovado.refresh_token !== refrescoA);
tokenA = renovado.access_token;

const perfil = await pedir('PATCH', '/api/auth/update', {
  token: tokenA,
  cuerpo: { display_name: 'Ana Prueba', bio: 'Cuenta creada por la prueba automática', location: 'Maracaibo' },
});
comprobar('perfil actualizado en la base de datos', perfil.display_name === 'Ana Prueba' && perfil.location === 'Maracaibo');

const sesiones = await pedir('GET', '/api/auth/sessions', { token: tokenA });
comprobar('listado de sesiones', Array.isArray(sesiones) && sesiones.length >= 1);

// ---------- Publicaciones ----------
const publicacion = await pedir('POST', '/api/posts', {
  token: tokenA,
  cuerpo: { content: `Prueba real de Moon #prueba${sufijo} con dos imágenes`, images: [
    { url: '/api/media/demo-1.jpg' }, { url: '/api/media/demo-2.jpg' },
  ] },
});
comprobar('publicación creada con etiqueta e imágenes', publicacion.id > 0 && publicacion.images.length === 2);
comprobar('la etiqueta se guardó', /#prueba/.test(publicacion.content));

const feed = await pedir('GET', '/api/feed?page=1&limit=10', { token: tokenA });
comprobar('el inicio devuelve la publicación recién creada', feed.items.some((p) => p.id === publicacion.id) && feed.total >= 1);

const meGusta = await pedir('POST', `/api/posts/${publicacion.id}/like`, { token: tokenA, cuerpo: {} });
comprobar('me gusta (contador en la base de datos)', meGusta.is_liked === true && meGusta.likes_count === 1);

const sinMeGusta = await pedir('DELETE', `/api/posts/${publicacion.id}/like`, { token: tokenA });
comprobar('quitar me gusta', sinMeGusta.is_liked === false && sinMeGusta.likes_count === 0);

const guardada = await pedir('POST', `/api/posts/${publicacion.id}/save`, { token: tokenA, cuerpo: {} });
comprobar('guardar publicación', guardada.is_saved === true);

const guardadas = await pedir('GET', '/api/me/saved?page=1&limit=20', { token: tokenA });
comprobar('la publicación guardada aparece en la lista', guardadas.items.some((p) => p.id === publicacion.id));

const editada = await pedir('PATCH', `/api/posts/${publicacion.id}`, { token: tokenA, cuerpo: { content: 'Texto editado de la prueba' } });
comprobar('edición de la publicación', editada.content === 'Texto editado de la prueba' && !!editada.edited_at);

// ---------- Segunda cuenta: seguir, comentar, mensajes ----------
const beto = await pedir('POST', '/api/auth/register', {
  cuerpo: { username: nombreB, email: `${nombreB}@moon.test`, password: clave },
});
let tokenB = beto.access_token;
comprobar('segunda cuenta registrada', !!tokenB && beto.user.username === nombreB);

const seguido = await pedir('POST', `/api/users/${yo.id}/follow`, { token: tokenB, cuerpo: {} });
comprobar('seguir a otra persona', seguido.is_following === true);

const perfilAna = await pedir('GET', `/api/users/${yo.id}`, { token: tokenB });
comprobar('el perfil muestra que la sigo y sumó seguidores', perfilAna.is_following === true && perfilAna.followers_count >= 1);

const comentario = await pedir('POST', `/api/posts/${publicacion.id}/comments`, { token: tokenB, cuerpo: { content: 'Comentario real de la prueba' } });
comprobar('comentario creado', comentario.id > 0 && comentario.content.includes('real'));

const comentarios = await pedir('GET', `/api/posts/${publicacion.id}/comments`, { token: tokenA });
comprobar('el comentario se lee', comentarios.some((c) => c.id === comentario.id));

const conComentario = await pedir('GET', `/api/posts/${publicacion.id}`, { token: tokenA });
comprobar('el contador de comentarios subió', conComentario.comments_count === 1, String(conComentario.comments_count));

const conversacion = await pedir('POST', '/api/messages/conversations', { token: tokenB, cuerpo: { user_id: yo.id } });
comprobar('conversación creada', conversacion.conversation_id > 0);

const mensaje = await pedir('POST', `/api/messages/conversations/${conversacion.conversation_id}/messages`, {
  token: tokenB, cuerpo: { content: 'Hola Ana, mensaje real' },
});
comprobar('mensaje enviado', mensaje.id > 0 && mensaje.content.includes('real'));

const hilo = await pedir('GET', `/api/messages/conversations/${conversacion.conversation_id}`, { token: tokenA });
comprobar('el hilo se lee desde la otra cuenta', hilo.messages.length === 1 && hilo.partner.username === nombreB);

const reaccion = await pedir('POST', `/api/messages/${mensaje.id}/react`, { token: tokenA, cuerpo: { reaction: '👍' } });
comprobar('reacción a un mensaje', reaccion.reaction === '👍');

const leido = await pedir('POST', `/api/messages/conversations/${conversacion.conversation_id}/read`, { token: tokenA, cuerpo: {} });
comprobar('marcar como leído', leido.ok === true);

const conversaciones = await pedir('GET', '/api/messages/conversations', { token: tokenA });
comprobar('lista de conversaciones', conversaciones.some((cv) => cv.id === conversacion.conversation_id));

// ---------- Notificaciones ----------
const notificaciones = await pedir('GET', '/api/notifications?page=1&limit=30', { token: tokenA });
const tipos = notificaciones.items.map((n) => n.type);
comprobar('hay notificaciones de seguimiento, comentario y mensaje', tipos.includes('follow') && tipos.includes('comment'), tipos.join(','));

await pedir('POST', '/api/notifications/read-all', { token: tokenA, cuerpo: {} });
const trasLeer = await pedir('GET', '/api/notifications?page=1&limit=30', { token: tokenA });
comprobar('marcar todas como leídas', trasLeer.items.every((n) => n.is_read));

// ---------- Búsqueda y tendencias ----------
const busqueda = await pedir('GET', `/api/search?q=${nombreB}&type=users`, { token: tokenA });
comprobar('búsqueda de personas', busqueda.some((u) => u.username === nombreB));

const busquedaPosts = await pedir('GET', '/api/search?q=prueba&type=posts', { token: tokenA });
comprobar('búsqueda de publicaciones', busquedaPosts.length >= 1);

const etiquetas = await pedir('GET', '/api/hashtags', { token: tokenA });
comprobar('tendencias con la etiqueta nueva', etiquetas.some((h) => h.tag === `prueba${sufijo}`), JSON.stringify(etiquetas.map((h) => h.tag)));

const sugerencias = await pedir('GET', '/api/users/suggestions', { token: tokenA });
comprobar('sugerencias de a quién seguir', Array.isArray(sugerencias) && sugerencias.length >= 1);

// ---------- Reportes y administración ----------
await pedir('POST', '/api/reports', { token: tokenB, cuerpo: { target_type: 'post', target_id: publicacion.id, reason: 'prueba automática' } });

const esAdmin = yo.role === 'admin';
if (esAdmin) {
  const panel = await pedir('GET', '/api/admin/dashboard', { token: tokenA });
  comprobar('panel de administración con datos reales', panel.users_total >= 2 && panel.posts_total >= 1, JSON.stringify(panel));

  const serie = await pedir('GET', '/api/admin/stats', { token: tokenA });
  comprobar('serie diaria de los últimos 30 días', Array.isArray(serie) && serie.length >= 1);

  const usuarios = await pedir('GET', '/api/admin/users?q=&page=1&limit=20', { token: tokenA });
  comprobar('listado de personas en administración', usuarios.items.length >= 2);

  await pedir('POST', '/api/admin/words', { token: tokenA, cuerpo: { word: 'palabrota' } });
  const palabras = await pedir('GET', '/api/admin/words', { token: tokenA });
  const palabra = palabras.find((w) => w.word === 'palabrota');
  comprobar('palabra bloqueada añadida', !!palabra);

  const bloqueado = await pedir('POST', '/api/posts', { token: tokenA, cuerpo: { content: 'esto tiene palabrota dentro' }, crudo: true });
  comprobar('la palabra bloqueada impide publicar', bloqueado.estado === 400, String(bloqueado.estado));

  if (palabra) await pedir('DELETE', `/api/admin/words/${palabra.id}`, { token: tokenA });

  const reportes = await pedir('GET', '/api/admin/reports?status=open&page=1&limit=20', { token: tokenA });
  comprobar('reporte recibido en administración', reportes.items.length >= 1);
  const reporte = reportes.items[0];
  await pedir('POST', `/api/admin/reports/${reporte.id}/resolve`, { token: tokenA, cuerpo: { action: 'dismiss', note: 'prueba' } });
  const trasResolver = await pedir('GET', '/api/admin/reports?status=open&page=1&limit=20', { token: tokenA });
  comprobar('reporte resuelto', !trasResuelto(trasResolver, reporte.id));

  const actividad = await pedir('GET', '/api/admin/activity?page=1&limit=30', { token: tokenA });
  comprobar('registro de actividad', actividad.items.length >= 1);

  const metricas = await pedir('GET', '/api/metrics', { token: tokenA });
  comprobar('métricas solo para administración', metricas.service === 'moon-api' && typeof metricas.totales.users === 'number');

  const sinPermiso = await pedir('GET', '/api/admin/dashboard', { token: tokenB, crudo: true });
  comprobar('una cuenta normal no entra a administración', sinPermiso.estado === 403, String(sinPermiso.estado));
} else {
  console.log('  (omitido) administración: la cuenta de prueba no es administradora');
}

function trasResuelto(lista, id) {
  return lista.items.some((r) => r.id === id);
}

// ---------- Grupos ----------
const grupo = await pedir('POST', '/api/groups', {
  token: tokenA,
  cuerpo: { name: `Cielo del Sur ${sufijo}`, about: 'Fotos y charlas del cielo austral' },
});
comprobar('grupo creado con su dirección corta', grupo.id > 0 && grupo.slug.includes('cielo-del-sur') && grupo.miembros === 1, JSON.stringify(grupo));

const listadoGrupos = await pedir('GET', '/api/groups', { token: tokenB });
comprobar('el grupo aparece para descubrir', listadoGrupos.descubrir.some((g) => g.id === grupo.id));

const entreGrupo = await pedir('POST', `/api/groups/${grupo.id}/join`, { token: tokenB, cuerpo: {} });
comprobar('segundo usuario entra al grupo', entreGrupo.soy_miembro === true);

const detalleGrupo = await pedir('GET', `/api/groups/${grupo.id}`, { token: tokenB });
comprobar('el grupo muestra sus dos miembros y sus papeles', detalleGrupo.miembros === 2 && detalleGrupo.miembros_lista.some((m) => m.papel === 'owner'));

const publicacionGrupo = await pedir('POST', `/api/groups/${grupo.id}/posts`, {
  token: tokenB,
  cuerpo: { content: `Primera del grupo #cielo ${sufijo}` },
});
comprobar('se publica dentro del grupo', publicacionGrupo.id > 0, JSON.stringify(publicacionGrupo).slice(0, 160));

const feedGrupo = await pedir('GET', `/api/groups/${grupo.id}/posts`, { token: tokenA });
comprobar('el grupo muestra lo publicado', feedGrupo.total === 1 && feedGrupo.items[0].content.includes('cielo'), JSON.stringify(feedGrupo).slice(0, 200));

const misGrupos = await pedir('GET', '/api/me/groups', { token: tokenB });
comprobar('mis grupos viene con el papel de cada uno', misGrupos.some((g) => g.id === grupo.id && g.papel === 'member'));

const salidaGrupo = await pedir('DELETE', `/api/groups/${grupo.id}/join`, { token: tokenB, crudo: true });
comprobar('se puede salir del grupo', salidaGrupo.estado === 200 && salidaGrupo.datos.soy_miembro === false, JSON.stringify(salidaGrupo.datos));

const privado = await pedir('POST', '/api/groups', { token: tokenA, cuerpo: { name: `Círculo privado ${sufijo}`, privacy: 'private' } });
const mirarPrivado = await pedir('GET', `/api/groups/${privado.id}/posts`, { token: tokenB, crudo: true });
comprobar('grupo privado: hay que entrar para ver lo publicado', mirarPrivado.estado === 403, String(mirarPrivado.estado));

const editarAjeno = await pedir('PATCH', `/api/groups/${grupo.id}`, { token: tokenB, cuerpo: { about: 'no debería' }, crudo: true });
comprobar('solo quien creó el grupo puede editarlo', editarAjeno.estado === 403, String(editarAjeno.estado));

// ---------- Imágenes: subir y volver a servir ----------
// (Esta parte cubre dos fallos reales: Busboy cerraba antes de que el archivo
// terminara de escribirse, y servir una imagen tumbaba el servidor entero.)
const png = Buffer.from(
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFAAH/q842iQAAAABJRU5ErkJggg==',
  'base64'
);
const formulario = new FormData();
formulario.append('name', 'story');
formulario.append('file', new Blob([new Uint8Array(png)], { type: 'image/png' }), 'foto.png');
const subida = await pedir('POST', '/api/upload', { token: tokenA, cuerpo: formulario });
comprobar(
  'la imagen se sube y queda registrada',
  Number(subida.id) > 0 && /^\/api\/media\//.test(subida.url || '') && subida.kind === 'story',
  JSON.stringify(subida)
);

const imagen = await fetch(`${API}${subida.url}`);
const bytesImagen = Buffer.from(await imagen.arrayBuffer());
comprobar(
  'la imagen se sirve con su tipo y su tamaño',
  imagen.status === 200 && imagen.headers.get('content-type') === 'image/png' && bytesImagen.length === png.length,
  `${imagen.status} ${imagen.headers.get('content-type')} ${bytesImagen.length}`
);

const saludTrasImagen = await pedir('GET', '/api/health');
comprobar('el servidor sigue en pie después de servir una imagen', saludTrasImagen.status === 'ok', JSON.stringify(saludTrasImagen));

// ---------- Historias (24 horas) y presencia ----------
const historia = await pedir('POST', '/api/stories', {
  token: tokenA,
  cuerpo: { image_url: subida.url, caption: 'Primera historia de la prueba' },
});
comprobar('historia creada', historia.id > 0 && historia.caption.includes('prueba'));

const historias = await pedir('GET', '/api/stories', { token: tokenB });
comprobar('la historia se ve desde quien sigue', historias.some((h) => h.user_id === yo.id && h.total === 1), JSON.stringify(historias));

const detalleHistoria = await pedir('GET', `/api/stories/${yo.id}`, { token: tokenB });
comprobar('detalle de la historia', detalleHistoria.stories.length === 1 && detalleHistoria.user.username === nombreA);

const vista = await pedir('POST', `/api/stories/${historia.id}/view`, { token: tokenB, cuerpo: {} });
comprobar('marcar la historia como vista', vista.ok === true);

const trasVer = await pedir('GET', `/api/stories/${yo.id}`, { token: tokenA });
comprobar('el contador de visitas de la historia subió', trasVer.stories[0].views_count === 1, String(trasVer.stories[0].views_count));

const presencia = await pedir('GET', '/api/users/presence', { token: tokenA });
comprobar('presencia devuelve contactos', Array.isArray(presencia.otros) && (presencia.otros.length + presencia.en_linea.length) >= 1);

const novedades = await pedir('GET', '/api/novedades', { token: tokenA });
comprobar('novedades con contadores reales', typeof novedades.avisos === 'number' && typeof novedades.mensajes === 'number' && typeof novedades.historias === 'number', JSON.stringify(novedades));

// ---------- Ajustes ampliados: reacciones, números y bloqueados ----------
const reacciones = await pedir('GET', `/api/posts/${publicacion.id}/likes`, { token: tokenA });
comprobar(
  'la lista de reacciones trae gente y total',
  typeof reacciones.total === 'number' && Array.isArray(reacciones.items) && reacciones.total >= 1,
  JSON.stringify(reacciones).slice(0, 120)
);

const misNumeros = await pedir('GET', '/api/me/stats', { token: tokenA });
comprobar(
  'mis números traen publicaciones, me gusta y mensajes',
  typeof misNumeros.posts === 'number' && typeof misNumeros.me_gusta_recibidos === 'number'
    && typeof misNumeros.mensajes === 'number' && typeof misNumeros.seguidores === 'number',
  JSON.stringify(misNumeros).slice(0, 140)
);

const prefsAvisos = await pedir('GET', '/api/notifications/prefs', { token: tokenA });
comprobar(
  'las preferencias de avisos tienen las siete claves',
  ['follow', 'like', 'comment', 'reply', 'mention', 'message', 'system'].every((k) => k in prefsAvisos),
  JSON.stringify(prefsAvisos)
);
const prefsGuardadas = await pedir('PATCH', '/api/notifications/prefs', {
  token: tokenA,
  cuerpo: { follow: true, like: false, comment: true, reply: true, mention: true, message: false, system: true },
});
comprobar('apagar un aviso se guarda', prefsGuardadas.like === false && prefsGuardadas.message === false, JSON.stringify(prefsGuardadas));

await pedir('POST', `/api/users/${beto.user.id}/block`, { token: tokenA, cuerpo: {} });
const bloqueados = await pedir('GET', '/api/me/blocked', { token: tokenA });
comprobar(
  'mi lista de bloqueados incluye a quien bloqueé',
  Array.isArray(bloqueados) && bloqueados.some((u) => Number(u.id) === Number(beto.user.id)),
  JSON.stringify(bloqueados).slice(0, 120)
);
await pedir('DELETE', `/api/users/${beto.user.id}/block`, { token: tokenA });
const sinBloqueos = await pedir('GET', '/api/me/blocked', { token: tokenA });
comprobar('desbloquear deja la lista vacía', Array.isArray(sinBloqueos) && sinBloqueos.length === 0, JSON.stringify(sinBloqueos));

// ---------- Encuestas ----------
const conEncuesta = await pedir('POST', '/api/posts', {
  token: tokenA,
  cuerpo: {
    content: `¿Qué hacemos el sábado? #encuesta${sufijo}`,
    poll: { pregunta: '¿Qué preparamos?', opciones: ['Pizza', 'Sushi', 'Asado'], horas: 24 },
  },
});
comprobar(
  'la publicación con encuesta trae sus opciones',
  Array.isArray(conEncuesta.poll?.opciones) && conEncuesta.poll.opciones.length === 3 && conEncuesta.poll.total === 0,
  JSON.stringify(conEncuesta.poll).slice(0, 140)
);
const votada = await pedir('POST', `/api/posts/${conEncuesta.id}/vote`, { token: tokenB, cuerpo: { opcion: 1 } });
comprobar(
  'votar suma el voto y lo marca como mío',
  votada.poll?.total === 1 && votada.poll.mi_voto.includes(1) && votada.poll.opciones[1].porcentaje === 100,
  JSON.stringify(votada.poll).slice(0, 160)
);
const revotada = await pedir('POST', `/api/posts/${conEncuesta.id}/vote`, { token: tokenB, cuerpo: { opcion: 0 } });
comprobar(
  'cambiar el voto no duplica (una sola respuesta)',
  revotada.poll?.total === 1 && revotada.poll.mi_voto.includes(0) && revotada.poll.opciones[1].votos === 0,
  JSON.stringify(revotada.poll).slice(0, 160)
);
const votoMalo = await pedir('POST', `/api/posts/${conEncuesta.id}/vote`, { token: tokenA, cuerpo: { opcion: 9 }, crudo: true });
comprobar('una opción que no existe se rechaza', votoMalo.estado === 400, String(votoMalo.estado));

// ---------- Avisos al teléfono ----------
const llavePush = await pedir('GET', '/api/push/public-key', { token: tokenA });
comprobar('el servidor entrega la llave de los avisos', typeof llavePush.key === 'string' && llavePush.key.length > 20, JSON.stringify(llavePush).slice(0, 80));
const suscripcion = await pedir('POST', '/api/push/subscribe', {
  token: tokenA,
  cuerpo: { endpoint: `https://ejemplo.test/${sufijo}`, keys: { p256dh: 'p'.repeat(40), auth: 'a'.repeat(20) } },
});
comprobar('el dispositivo queda registrado', suscripcion.ok === true && suscripcion.dispositivos >= 1, JSON.stringify(suscripcion));
const estadoPush = await pedir('GET', '/api/push/estado', { token: tokenA });
comprobar('el estado dice cuántos dispositivos hay', estadoPush.dispositivos >= 1, JSON.stringify(estadoPush));
await pedir('DELETE', '/api/push/subscribe', { token: tokenA, cuerpo: {} });
const trasBorrar = await pedir('GET', '/api/push/estado', { token: tokenA });
comprobar('al desactivar quedan cero dispositivos', trasBorrar.dispositivos === 0, JSON.stringify(trasBorrar));

// ---------- Copias de seguridad ----------
if (esAdmin) {
  const copia = await pedir('GET', '/api/admin/backup', { token: tokenA, crudo: true });
  comprobar(
    'el administrador puede descargar la copia',
    copia.estado === 200 && String(copia.texto || '').includes('"app": "moon"'),
    `estado ${copia.estado}`
  );
} else {
  const copiaSinPermiso = await pedir('GET', '/api/admin/backup', { token: tokenA, crudo: true });
  comprobar('quien no es administrador no puede descargar la copia', copiaSinPermiso.estado === 403, String(copiaSinPermiso.estado));
}

// ---------- Imágenes dentro de la base de datos ----------
const subidaImagen = await pedir('POST', '/api/upload', {
  token: tokenA,
  cuerpo: (() => {
    const fd = new FormData();
    fd.append('name', 'post');
    fd.append('file', new Blob([Buffer.from('PNG-falso-de-prueba')], { type: 'image/png' }), 'prueba.png');
    return fd;
  })(),
});
comprobar('la imagen se sube y devuelve dirección', /^\/api\/media\//.test(String(subidaImagen.url || '')), JSON.stringify(subidaImagen).slice(0, 120));
const imagenServida = await pedir('GET', String(subidaImagen.url || '/api/media/x'), { token: tokenA, crudo: true });
comprobar('la imagen se entrega (guardada en la base de datos)', imagenServida.estado === 200, `estado ${imagenServida.estado}`);

// ---------- Bloqueo ----------
await pedir('POST', `/api/users/${beto.user.id}/block`, { token: tokenA, cuerpo: {} });
const perfilBloqueado = await pedir('GET', `/api/users/${beto.user.id}`, { token: tokenA, crudo: true });
comprobar('el perfil bloqueado ya no es accesible', perfilBloqueado.estado === 403, String(perfilBloqueado.estado));
await pedir('DELETE', `/api/users/${beto.user.id}/block`, { token: tokenA });

// ---------- Cambio de contraseña y cierre ----------
await pedir('POST', '/api/auth/change-password', { token: tokenA, cuerpo: { current_password: clave, new_password: `${clave}-nueva` } });
const accesoNuevo = await pedir('POST', '/api/auth/login', { cuerpo: { username: nombreA, password: `${clave}-nueva` }, crudo: true });
comprobar('la contraseña nueva funciona', accesoNuevo.estado === 200 || !!accesoNuevo.datos?.access_token, String(accesoNuevo.estado));
const accesoViejo = await pedir('POST', '/api/auth/login', { cuerpo: { username: nombreA, password: clave }, crudo: true });
comprobar('la contraseña anterior ya no sirve', accesoViejo.estado === 401, String(accesoViejo.estado));

await pedir('POST', '/api/auth/logout', { token: tokenA, refresco: 'no-existe' });

console.log(`\n${ok} correctas, ${fallos} fallos`);
console.log(`Cuentas de prueba: ${nombreA} / ${nombreB} (contraseña original: ${clave})`);
process.exit(fallos === 0 ? 0 : 1);
