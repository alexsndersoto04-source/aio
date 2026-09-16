// Moon — Prueba real de la tanda 3 (grupos completos)
// Uso: API=http://127.0.0.1:3012 node test/prueba-tanda3.mjs
const API = (process.env.API || 'http://127.0.0.1:3000').replace(/\/$/, '');
let ok = 0, mal = 0;
const comprobar = (nombre, cond, extra = '') => {
  if (cond) { ok++; console.log(`  ok    ${nombre}`); }
  else { mal++; console.log(`  FALLO ${nombre} ${extra}`); }
};
async function pedir(metodo, ruta, { cuerpo, token } = {}) {
  const cabeceras = {};
  const esFormulario = typeof FormData !== 'undefined' && cuerpo instanceof FormData;
  if (!esFormulario && cuerpo !== undefined) cabeceras['content-type'] = 'application/json';
  if (token) cabeceras.Authorization = `Bearer ${token}`;
  const carga = cuerpo === undefined ? undefined : (esFormulario ? cuerpo : JSON.stringify(cuerpo));
  const r = await fetch(API + ruta, { method: metodo, headers: cabeceras, body: carga });
  let d = null; try { d = await r.json(); } catch { d = null; }
  return { estado: r.status, d };
}
const sello = Date.now().toString().slice(-6);
const A = `t3a${sello}`, B = `t3b${sello}`, C = `t3c${sello}`;
const reg = async (u) => (await pedir('POST', '/api/auth/register', { cuerpo: { username: u, email: `${u}@moon.test`, password: 'ClaveSegura123' } })).d;
const a = await reg(A), b = await reg(B), c = await reg(C);
const tA = a.access_token, tB = b.access_token, tC = c.access_token;
comprobar('tres cuentas de prueba', !!tA && !!tB && !!tC);

const g = await pedir('POST', '/api/groups', { cuerpo: { name: `Equipo ${sello}`, about: 'Grupo de prueba de la tanda 3', privacy: 'public' }, token: tA });
const gid = g.d?.id;
comprobar('grupo creado', !!gid, JSON.stringify(g.d));
comprobar('nace con reglas vacías y sin anuncio', g.d?.rules === '' && g.d?.announcement === '', JSON.stringify([g.d?.rules, g.d?.announcement]));
await pedir('POST', `/api/groups/${gid}/join`, { token: tB });
await pedir('POST', `/api/groups/${gid}/join`, { token: tC });

// ---------- 1) Reglas y anuncio fijado ----------
const r1 = await pedir('PATCH', `/api/groups/${gid}/rules`, { cuerpo: { rules: '1. Respeto\n2. Nada de spam\n3. Un tema por publicación', announcement: 'Reunión el viernes a las 5' }, token: tA });
comprobar('el dueño guarda reglas y anuncio', r1.estado === 200 && r1.d?.announcement.includes('viernes'), JSON.stringify(r1.d));
const rAjeno = await pedir('PATCH', `/api/groups/${gid}/rules`, { cuerpo: { rules: 'mías' }, token: tB });
comprobar('un miembro cualquiera no cambia las reglas', rAjeno.estado === 403, String(rAjeno.estado));
const rLectura = await pedir('GET', `/api/groups/${gid}/rules`, { token: tB });
comprobar('los miembros leen reglas y anuncio', rLectura.d?.rules.includes('Respeto') && rLectura.d?.announcement.includes('viernes'), JSON.stringify(rLectura.d));
const detalle = await pedir('GET', `/api/groups/${gid}`, { token: tB });
comprobar('el anuncio viaja al abrir el grupo', detalle.d?.announcement.includes('viernes'), JSON.stringify(detalle.d?.announcement));
comprobar('el detalle dice si tengo mando', detalle.d?.mando === false && (await pedir('GET', `/api/groups/${gid}`, { token: tA })).d?.mando === true);
const anuncioFuera = await pedir('PATCH', `/api/groups/${gid}/rules`, { cuerpo: { announcement: '' }, token: tA });
comprobar('el anuncio se puede quitar', anuncioFuera.d?.announcement === '' && anuncioFuera.d?.announcement_at === null, JSON.stringify(anuncioFuera.d));
await pedir('PATCH', `/api/groups/${gid}/rules`, { cuerpo: { announcement: 'Trae el portátil' }, token: tA });

// ---------- 2) Eventos ----------
const ev = await pedir('POST', `/api/groups/${gid}/events`, { cuerpo: { title: 'Repaso del trimestre', about: 'Vemos números', place: 'Sala 3', starts_at: new Date(Date.now() + 86400000).toISOString() }, token: tA });
comprobar('el dueño crea un evento', ev.estado === 200 && ev.d?.title === 'Repaso del trimestre', `${ev.estado} ${JSON.stringify(ev.d)}`);
comprobar('quien lo crea va de una', ev.d?.voy === 1 && ev.d?.mi_estado === 'voy', JSON.stringify([ev.d?.voy, ev.d?.mi_estado]));
const evAjeno = await pedir('POST', `/api/groups/${gid}/events`, { cuerpo: { title: 'Fiesta sorpresa', starts_at: new Date().toISOString() }, token: tB });
comprobar('un miembro sin mando no crea eventos', evAjeno.estado === 403, String(evAjeno.estado));
const evSinFecha = await pedir('POST', `/api/groups/${gid}/events`, { cuerpo: { title: 'Sin fecha' }, token: tA });
comprobar('un evento sin fecha se rechaza', evSinFecha.estado === 400, String(evSinFecha.estado));
const asistir = await pedir('POST', `/api/groups/${gid}/events/${ev.d.id}/asistir`, { cuerpo: { estado: 'quizas' }, token: tB });
comprobar('otro responde «quizás»', asistir.d?.quizas === 1 && asistir.d?.mi_estado === 'quizas', JSON.stringify(asistir.d));
const tambien = await pedir('POST', `/api/groups/${gid}/events/${ev.d.id}/asistir`, { cuerpo: { estado: 'voy' }, token: tC });
comprobar('otro más responde «voy»', tambien.d?.voy === 2, JSON.stringify(tambien.d?.voy));
const quitar = await pedir('POST', `/api/groups/${gid}/events/${ev.d.id}/asistir`, { cuerpo: { estado: 'voy' }, token: tC });
comprobar('volver a pulsar quita la respuesta', quitar.d?.voy === 1 && quitar.d?.mi_estado === '', JSON.stringify([quitar.d?.voy, quitar.d?.mi_estado]));
const gente = await pedir('GET', `/api/groups/${gid}/events/${ev.d.id}/asistentes`, { token: tB });
comprobar('se ve quién va y quién quizás', (gente.d?.asistentes || []).length === 2, JSON.stringify(gente.d?.asistentes));
const listaEv = await pedir('GET', `/api/groups/${gid}/events`, { token: tC });
comprobar('el evento sale en la lista del grupo', (listaEv.d?.eventos || []).some((x) => x.id === ev.d.id));
const evBorradoAjeno = await pedir('DELETE', `/api/groups/${gid}/events/${ev.d.id}`, { token: tB });
comprobar('nadie más borra un evento que no creó', evBorradoAjeno.estado === 403, String(evBorradoAjeno.estado));

// ---------- 3) Archivos ----------
const formulario = new FormData();
formulario.append('name', 'file');
formulario.append('file', new Blob([Buffer.from('PNG-falso-del-grupo')], { type: 'image/png' }), 'acta.png');
const subida = await pedir('POST', '/api/upload', { cuerpo: formulario, token: tA });
const url = subida.d?.url || '';
const arch = await pedir('POST', `/api/groups/${gid}/files`, { cuerpo: { nombre: 'acta.png', url, tipo: 'image/png', peso: 2048 }, token: tB });
comprobar('un miembro comparte un archivo', arch.estado === 200 && arch.d?.nombre === 'acta.png', `${arch.estado} ${JSON.stringify(arch.d)}`);
const archMalo = await pedir('POST', `/api/groups/${gid}/files`, { cuerpo: { nombre: 'x.pdf', url: 'http://otra-web/x.pdf' }, token: tB });
comprobar('solo se aceptan archivos subidos a la app', archMalo.estado === 400, String(archMalo.estado));
const archLista = await pedir('GET', `/api/groups/${gid}/files`, { token: tC });
comprobar('el archivo aparece en la lista', (archLista.d?.archivos || []).some((x) => x.id === arch.d.id), JSON.stringify(archLista.d?.archivos?.length));
comprobar('quien lo subió puede borrarlo, otro no', (archLista.d?.archivos || [])[0]?.puedo_borrar === false);
const archBorraAjeno = await pedir('DELETE', `/api/groups/${gid}/files/${arch.d.id}`, { token: tC });
comprobar('borrar el archivo de otro falla', archBorraAjeno.estado === 403, String(archBorraAjeno.estado));
const archBorraDueno = await pedir('DELETE', `/api/groups/${gid}/files/${arch.d.id}`, { token: tA });
comprobar('quien administra sí puede quitarlo', archBorraDueno.estado === 200, String(archBorraDueno.estado));

// ---------- 4) Preguntas para entrar ----------
const preguntas = await pedir('PATCH', `/api/groups/${gid}/rules`, { cuerpo: { join_questions: ['¿Por qué quieres entrar?', '¿Qué aportas?'] }, token: tA });
comprobar('el dueño pone preguntas de entrada', (preguntas.d?.join_questions || []).length === 2, JSON.stringify(preguntas.d?.join_questions));
const D = `t3d${sello}`;
const d1 = await reg(D);
const tD = d1.access_token;
const preg = await pedir('GET', `/api/groups/${gid}/preguntas`, { token: tD });
comprobar('quien no está ve que hay preguntas', preg.d?.pregunta === true && preg.d?.preguntas.length === 2, JSON.stringify(preg.d));
const sinRespuestas = await pedir('POST', `/api/groups/${gid}/solicitar`, { cuerpo: { answers: ['Porque sí'] }, token: tD });
comprobar('sin contestar todo no entra', sinRespuestas.estado === 400, String(sinRespuestas.estado));
const solicitud = await pedir('POST', `/api/groups/${gid}/solicitar`, { cuerpo: { answers: ['Me interesa el tema', 'Traigo datos'] }, token: tD });
comprobar('la solicitud queda pendiente', solicitud.d?.estado === 'pendiente' && !!solicitud.d?.solicitud_id, JSON.stringify(solicitud.d));
const repetida = await pedir('POST', `/api/groups/${gid}/solicitar`, { cuerpo: { answers: ['Otra vez', 'Otra'] }, token: tD });
comprobar('no se puede pedir dos veces seguidas', repetida.d?.repetida === true, JSON.stringify(repetida.d));
const solicitudes = await pedir('GET', `/api/groups/${gid}/solicitudes`, { token: tA });
comprobar('el dueño ve la solicitud con sus respuestas', (solicitudes.d?.solicitudes || []).some((s) => s.user_id === d1.user.id && s.answers.length === 2), JSON.stringify(solicitudes.d?.solicitudes));
const solicitudesAjenas = await pedir('GET', `/api/groups/${gid}/solicitudes`, { token: tB });
comprobar('un miembro sin mando no ve las solicitudes', solicitudesAjenas.estado === 403, String(solicitudesAjenas.estado));
const rechazar = await pedir('POST', `/api/groups/${gid}/solicitudes/${solicitud.d.solicitud_id}`, { cuerpo: { aprobar: false }, token: tA });
comprobar('el dueño puede rechazar', rechazar.d?.aprobada === false, JSON.stringify(rechazar.d));
const dentroTrasRechazo = await pedir('GET', `/api/groups/${gid}`, { token: tD });
comprobar('quien fue rechazado sigue fuera', dentroTrasRechazo.d?.soy_miembro === false, JSON.stringify(dentroTrasRechazo.d?.soy_miembro));
const solicitud2 = await pedir('POST', `/api/groups/${gid}/solicitar`, { cuerpo: { answers: ['Ahora con más ganas', 'Paciencia'] }, token: tD });
const aprobar = await pedir('POST', `/api/groups/${gid}/solicitudes/${solicitud2.d.solicitud_id}`, { cuerpo: { aprobar: true }, token: tA });
comprobar('el dueño puede aceptar', aprobar.d?.aprobada === true, JSON.stringify(aprobar.d));
const dentro = await pedir('GET', `/api/groups/${gid}`, { token: tD });
comprobar('quien fue aceptado ya es miembro', dentro.d?.soy_miembro === true, JSON.stringify(dentro.d?.soy_miembro));
const grupoSinPreguntas = await pedir('PATCH', `/api/groups/${gid}/rules`, { cuerpo: { join_questions: [] }, token: tA });
comprobar('las preguntas se pueden quitar', (grupoSinPreguntas.d?.join_questions || []).length === 0);

// ---------- 5) Sanciones y registro ----------
const aviso = await pedir('POST', `/api/groups/${gid}/sanciones`, { cuerpo: { user_id: b.user.id, tipo: 'aviso', motivo: 'Publicó dos veces lo mismo' }, token: tA });
comprobar('el dueño pone un aviso', aviso.estado === 200 && aviso.d?.tipo === 'aviso', `${aviso.estado} ${JSON.stringify(aviso.d)}`);
const silencio = await pedir('POST', `/api/groups/${gid}/sanciones`, { cuerpo: { user_id: b.user.id, tipo: 'silencio', motivo: 'Insistió', minutos: 30 }, token: tA });
comprobar('el silencio queda con hora de fin', silencio.d?.tipo === 'silencio' && !!silencio.d?.hasta, JSON.stringify(silencio.d));
const postSilenciado = await pedir('POST', `/api/groups/${gid}/posts`, { cuerpo: { content: 'Quiero publicar igual' }, token: tB });
comprobar('en silencio no se publica en el muro', postSilenciado.estado === 403 && postSilenciado.d?.error?.code !== undefined || postSilenciado.estado === 403, `${postSilenciado.estado}`);
const chatSilenciado = await pedir('POST', `/api/groups/${gid}/messages`, { cuerpo: { content: 'Ni en el chat' }, token: tB });
comprobar('en silencio tampoco se escribe en el chat', chatSilenciado.estado === 403, String(chatSilenciado.estado));
const sancionAjeno = await pedir('POST', `/api/groups/${gid}/sanciones`, { cuerpo: { user_id: c.user.id, tipo: 'aviso' }, token: tB });
comprobar('un miembro sin mando no sanciona', sancionAjeno.estado === 403, String(sancionAjeno.estado));
const miSancion = await pedir('GET', `/api/groups/${gid}/sanciones`, { token: tB });
comprobar('cada quien ve sus sanciones', (miSancion.d?.sanciones || []).length >= 2 && miSancion.d?.mando === false, JSON.stringify(miSancion.d?.sanciones?.length));
const sancionesDueno = await pedir('GET', `/api/groups/${gid}/sanciones`, { token: tA });
comprobar('el dueño ve todas', sancionesDueno.d?.mando === true && (sancionesDueno.d?.sanciones || []).length >= 2);
const perdon = await pedir('DELETE', `/api/groups/${gid}/sanciones/${silencio.d.id}`, { token: tA });
comprobar('el silencio se puede levantar', perdon.estado === 200, String(perdon.estado));
const postLibre = await pedir('POST', `/api/groups/${gid}/posts`, { cuerpo: { content: 'Ahora sí puedo publicar' }, token: tB });
comprobar('sin silencio se publica otra vez', postLibre.estado === 200 || postLibre.estado === 201, String(postLibre.estado));
const registro = await pedir('GET', `/api/groups/${gid}/registro`, { token: tA });
const hayAcciones = (registro.d?.registro || []).map((r) => r.accion);
comprobar('el registro guarda lo que pasó', hayAcciones.includes('evento_creado') && hayAcciones.includes('sancion_aviso') && hayAcciones.includes('solicitud_aprobada'), JSON.stringify(hayAcciones));
const registroAjeno = await pedir('GET', `/api/groups/${gid}/registro`, { token: tB });
comprobar('el registro es solo para quien administra', registroAjeno.estado === 403, String(registroAjeno.estado));
const expulsion = await pedir('POST', `/api/groups/${gid}/sanciones`, { cuerpo: { user_id: c.user.id, tipo: 'expulsion', motivo: 'Se fue del tema' }, token: tA });
comprobar('la expulsión saca a la persona', expulsion.estado === 200, String(expulsion.estado));
const fuera = await pedir('GET', `/api/groups/${gid}`, { token: tC });
comprobar('quien fue expulsado ya no está', fuera.d?.soy_miembro === false, JSON.stringify(fuera.d?.soy_miembro));
const aUnoMismo = await pedir('POST', `/api/groups/${gid}/sanciones`, { cuerpo: { user_id: a.user.id, tipo: 'aviso' }, token: tA });
comprobar('no te sancionas a ti mismo', aUnoMismo.estado === 409, String(aUnoMismo.estado));

// ---------- 6) Lo privado no se filtra ----------
const priv = await pedir('POST', '/api/groups', { cuerpo: { name: `Cerrado ${sello}`, privacy: 'private' }, token: tA });
const pid = priv.d?.id;
const E = `t3e${sello}`;
const e1 = await reg(E);
const tE = e1.access_token;
const verEventos = await pedir('GET', `/api/groups/${pid}/events`, { token: tE });
comprobar('en un grupo privado, fuera no se ven los eventos', verEventos.estado === 403, String(verEventos.estado));
const verArchivos = await pedir('GET', `/api/groups/${pid}/files`, { token: tE });
comprobar('ni los archivos', verArchivos.estado === 403, String(verArchivos.estado));
const verReglas = await pedir('GET', `/api/groups/${pid}/rules`, { token: tE });
comprobar('ni las reglas', verReglas.estado === 403, String(verReglas.estado));

console.log(`\n${mal === 0 ? 'TODO CORRECTO' : 'HAY FALLOS'} — ${ok} bien, ${mal} mal`);
process.exit(mal === 0 ? 0 : 1);
