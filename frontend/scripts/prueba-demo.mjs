// Prueba del modo demostración sin navegador.
//
// Recorre las mismas llamadas que hace la aplicación y comprueba que la
// respuesta tiene la forma esperada. Uso, desde `frontend/`:
//
//   node scripts/prueba-demo.mjs

import { responder } from '../src/demo.js';

let fallos = 0;

function comprobar(titulo, condicion, detalle = '') {
  if (condicion) {
    console.log(`  ok   ${titulo}`);
  } else {
    fallos += 1;
    console.log(`  FALLA ${titulo} ${detalle}`);
  }
}

const esObjeto = (v) => v !== null && typeof v === 'object';
const esLista = (v) => Array.isArray(v);

// [método, ruta, cuerpo, comprobación]
const casos = [
  ['POST', '/api/auth/login', { username: 'alice', password: 'demo' }, (d) => d.access_token && d.user?.username === 'alice'],
  ['POST', '/api/auth/register', { username: 'nuevo', email: 'n@moon.test', password: 'x' }, (d) => !!d.access_token],
  ['POST', '/api/auth/refresh', { refresh_token: 'demo' }, (d) => !!d.access_token],
  ['POST', '/api/auth/logout', {}, () => true],
  ['GET', '/api/auth/me', null, (d) => d.username === 'alice' && typeof d.followers_count === 'number'],
  ['PATCH', '/api/auth/update', { display_name: 'Alice Márquez' }, (d) => d.display_name === 'Alice Márquez'],
  ['PATCH', '/api/auth/privacy', { is_private: false }, () => true],
  ['GET', '/api/auth/sessions', null, esLista],
  ['DELETE', '/api/auth/sessions/s1', null, () => true],
  ['POST', '/api/auth/sessions-all', {}, () => true],
  ['POST', '/api/auth/change-password', { current_password: 'a', new_password: 'b' }, () => true],
  ['POST', '/api/auth/2fa/enable', { password: 'x' }, (d) => !!d.otpauth_url],
  ['POST', '/api/auth/2fa/confirm', { temp_token: 't', code: '123456' }, () => true],
  ['POST', '/api/auth/2fa/disable', { password: 'x' }, () => true],
  ['POST', '/api/auth/recovery/request', { email: 'a@b.c' }, () => true],
  ['POST', '/api/auth/recovery/verify', { token: 't', new_password: 'x' }, () => true],
  ['DELETE', '/api/auth/account', { password: 'x' }, () => true],

  ['GET', '/api/feed?page=1&limit=10', null, (d) => esLista(d.items) && d.items.length > 0 && typeof d.total === 'number'],
  ['GET', '/api/feed/trending?page=1&limit=10', null, (d) => esLista(d.items)],
  ['GET', '/api/feed/latest?page=1&limit=10', null, (d) => esLista(d.items)],
  ['GET', '/api/posts/101', null, (d) => d.id === 101 && !!d.author_username],
  ['POST', '/api/posts/101/like', {}, (d) => d.is_liked === true && d.likes_count > 0],
  ['DELETE', '/api/posts/101/like', null, (d) => d.is_liked === false],
  ['POST', '/api/posts/101/save', {}, (d) => d.is_saved === true],
  ['POST', '/api/posts', { content: 'Prueba', images: [] }, (d) => d.content === 'Prueba' && d.is_mine === true],
  ['GET', '/api/posts/101/comments', null, (d) => esLista(d) && d[0]?.username],
  ['POST', '/api/posts/101/comments', { content: 'Hola' }, (d) => d.content === 'Hola' && !!d.username],
  ['PATCH', '/api/posts/101', { content: 'Editada' }, (d) => d.content === 'Editada'],
  ['GET', '/api/me/saved?page=1&limit=20', null, (d) => esLista(d.items) && d.items.length > 0],
  ['GET', '/api/users/1/posts?page=1&limit=20', null, (d) => esLista(d.items) && d.items.length > 0],

  ['GET', '/api/users/2', null, (d) => d.username === 'bruno' && typeof d.is_following === 'boolean'],
  ['GET', '/api/users/2/posts?page=1&limit=20', null, (d) => esLista(d.items) && d.items.length > 0],
  ['POST', '/api/users/2/follow', {}, (d) => d.is_following === true],
  ['DELETE', '/api/users/2/follow', null, (d) => d.is_following === false],
  ['POST', '/api/users/2/block', {}, (d) => d.is_blocked === true],
  ['DELETE', '/api/users/2/block', null, (d) => d.is_blocked === false],
  ['GET', '/api/users/suggestions', null, (d) => esLista(d) && d.length >= 2 && !!d[0].username],
  ['GET', '/api/search?q=bruno&type=users', null, (d) => esLista(d) && d[0].username === 'bruno'],
  ['GET', '/api/search?q=dise%C3%B1o&type=posts', null, (d) => esLista(d) && d.length > 0],
  ['GET', '/api/hashtags', null, (d) => esLista(d) && !!d[0].tag && typeof d[0].posts_count === 'number'],

  ['GET', '/api/notifications?page=1&limit=30', null, (d) => esLista(d.items) && !!d.items[0].from_username && 'is_read' in d.items[0]],
  ['POST', '/api/notifications/11/read', {}, () => true],
  ['POST', '/api/notifications/read-all', {}, () => true],

  ['GET', '/api/messages/conversations', null, (d) => esLista(d) && !!d[0].username && typeof d[0].unread === 'number'],
  ['POST', '/api/messages/conversations', { user_id: 4 }, (d) => typeof d.conversation_id === 'number'],
  ['GET', '/api/messages/conversations/1', null, (d) => esObjeto(d) && esLista(d.messages) && !!d.partner?.username],
  ['POST', '/api/messages/conversations/1/messages', { content: 'Hola' }, (d) => !!d.id && d.content === 'Hola'],
  ['POST', '/api/messages/conversations/1/read', {}, () => true],
  ['POST', '/api/messages/1/react', { reaction: '👍' }, () => true],
  ['DELETE', '/api/messages/1', null, () => true],

  ['POST', '/api/reports', { target_type: 'post', target_id: 101, reason: 'spam' }, () => true],
  ['POST', '/api/upload', null, (d) => !!d.url],

  ['GET', '/api/admin/dashboard', null, (d) => typeof d.users_total === 'number' && typeof d.reports_open === 'number'],
  ['GET', '/api/admin/stats', null, (d) => esLista(d) && d.length === 30 && typeof d[0].new_users === 'number'],
  ['GET', '/api/admin/users?q=&page=1&limit=20', null, (d) => esLista(d.items) && !!d.items[0].username],
  ['GET', '/api/admin/users?q=bru&page=1&limit=20', null, (d) => esLista(d.items)],
  ['POST', '/api/admin/users/2/suspend', { reason: 'x' }, () => true],
  ['POST', '/api/admin/users/2/activate', {}, () => true],
  ['POST', '/api/admin/users/2/verify', { verified: true }, () => true],
  ['GET', '/api/admin/reports?status=open&page=1&limit=20', null, (d) => esLista(d.items) && !!d.items[0].reason],
  ['POST', '/api/admin/reports/1/resolve', { action: 'remove', note: '' }, () => true],
  ['GET', '/api/admin/words', null, (d) => esLista(d) && !!d[0].word],
  ['POST', '/api/admin/words', { word: 'prueba' }, () => true],
  ['DELETE', '/api/admin/words/1', null, () => true],
  ['GET', '/api/admin/activity?page=1&limit=30', null, (d) => esLista(d.items)],
];

console.log(`Prueba del modo demostración: ${casos.length} llamadas\n`);

for (const [metodo, ruta, cuerpo, revisar] of casos) {
  let resultado;
  try {
    resultado = responder(metodo, ruta, cuerpo);
  } catch (e) {
    fallos += 1;
    console.log(`  FALLA ${metodo} ${ruta} → excepción: ${e.message}`);
    continue;
  }
  const titulo = `${metodo} ${ruta}`;
  if (resultado.status !== 200) {
    fallos += 1;
    console.log(`  FALLA ${titulo} → estado ${resultado.status}`);
    continue;
  }
  let bien = false;
  try {
    bien = revisar(resultado.data);
  } catch (e) {
    console.log(`  FALLA ${titulo} → al comprobar: ${e.message}`);
  }
  comprobar(titulo, bien, `→ ${JSON.stringify(resultado.data).slice(0, 120)}`);
}

console.log(`\n${fallos === 0 ? 'TODO CORRECTO' : `${fallos} fallos`}`);
process.exit(fallos === 0 ? 0 : 1);
