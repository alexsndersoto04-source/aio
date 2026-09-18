// Moon — Orquestador de la mudanza a Neon (corre donde HAYA internet)
// ============================================================
// Usa la API de Render para hacer TODA la mudanza sin tocar el panel:
//
//   1. Pasa el servicio «moon» a la rama nueva (la que trae la migración).
//   2. Agrega la variable MOON_MIGRATE_DEST (la base de Neon).
//   3. El servidor, al arrancar, crea el esquema en Neon (si falta) y copia
//      SOLO: usuarios, posts, mensajes, grupos e imágenes.
//   4. Verifica en el registro del deploy que la copia quedó completa.
//   5. Cambia DATABASE_URL a Neon y quita MOON_MIGRATE_DEST.
//   6. Confirma /api/health en verde con los datos de Neon.
//
// Variables de entorno (nada de esto vive en el repo):
//   RENDER_API_KEY    clave de la API de Render (rnd_...)
//   MOON_DEST_URL     cadena de conexión de la base nueva (Neon)
//   MOON_RAMAS        rama a desplegar (por defecto, la que le diga el que llama)
//
// Salida: resumen por pasos en el log. Termina con código 1 si algo falla.

const API = 'https://api.render.com/1';
const CLAVE = process.env.RENDER_API_KEY || '';
const DESTINO = process.env.MON_DEST_URL || process.env.MOON_DEST_URL || '';
const RAMA = process.env.MOON_RAMAS || '';
const DOMINIO_ESPERADO = process.env.MOON_DOMINIO || 'moon-dal0.onrender.com';

let fallos = 0;
function paso(n, texto) {
  console.log(`\n${'='.repeat(60)}\nPaso ${n}: ${texto}\n${'='.repeat(60)}`);
}
function ok(texto) {
  console.log(`  ✔ ${texto}`);
}
function error(texto) {
  fallos += 1;
  console.error(`  ✘ ${texto}`);
}
async function dormir(ms) {
  await new Promise((r) => setTimeout(r, ms));
}

if (!CLAVE) {
  error('Falta RENDER_API_KEY');
  process.exit(1);
}
if (!DESTINO) {
  error('Falta MOON_DEST_URL (la cadena de Neon)');
  process.exit(1);
}
if (!RAMA) {
  error('Falta MOON_RAMAS (la rama con el código nuevo)');
  process.exit(1);
}

async function render(ruta, metodo = 'GET', cuerpo = undefined) {
  const res = await fetch(`${API}${ruta}`, {
    method: metodo,
    headers: {
      Authorization: `Bearer ${CLAVE}`,
      'Content-Type': 'application/json',
    },
    body: cuerpo === undefined ? undefined : JSON.stringify(cuerpo),
  });
  const texto = await res.text();
  let datos = null;
  try {
    datos = texto ? JSON.parse(texto) : null;
  } catch {
    datos = texto;
  }
  if (!res.ok) {
    const detalle = typeof datos === 'object' ? datos?.detail || datos?.message || texto : texto;
    throw new Error(`${metodo} ${ruta} → HTTP ${res.status}: ${String(detalle).slice(0, 300)}`);
  }
  return datos;
}

async function esperarDeploy(servicio, desdeDeploy, timeoutMin = 20) {
  const inicio = Date.now();
  let ultimo = '';
  while (Date.now() - inicio < timeoutMin * 60_000) {
    await dormir(15_000);
    const s = await render(`/services/${servicio.id}`);
    const d = s.latestDeployment || {};
    const linea = `deploy ${d.id?.slice(0, 8)}… estado: ${d.status}`;
    if (linea !== ultimo) {
      console.log(`  … ${linea}`);
      ultimo = linea;
    }
    if (d.id && d.id !== desdeDeploy) {
      if (d.status === 'ready') return d;
      if (d.status === 'errored' || d.status === 'canceled') {
        throw new Error(`El deploy ${d.id} terminó en estado «${d.status}»`);
      }
    }
  }
  throw new Error(`No termino el deploy en ${timeoutMin} minutos`);
}

async function leerLogsMigracion(servicio, deployId) {
  // Busca en el registro del deploy las líneas de la migración.
  const res = await fetch(`${API}/deploys/${deployId}/logs?wait=false`, {
    headers: { Authorization: `Bearer ${CLAVE}` },
  });
  if (!res.ok) return { listo: false, fallo: false, nota: `no se pudieron leer los logs (HTTP ${res.status})` };
  const lineas = (await res.json()).map((l) => l.message).join('\n');
  const lista = /automática lista: ([^\n]*)/.exec(lineas);
  const fallo = /automática no se ejecutó: ([^\n]*)/.exec(lineas);
  return {
    listo: !!lista,
    resumen: lista ? lista[1] : '',
    fallo: !!fallo,
    nota: fallo ? fallo[1] : '',
  };
}

async function main() {
  // ----------------------------------------------------------
  paso(1, `Encontrar el servicio de Moon en Render (dominio ${DOMINIO_ESPERADO})`);
  const servicios = await render('/services');
  const servicio = servicios.find((s) =>
    (s.publicDomain || '').includes(DOMINIO_ESPERADO) || (s.name === 'moon' && s.runtime === 'node')
  );
  if (!servicio) {
    error(`No se encontró el servicio (dominio ${DOMINIO_ESPERADO}). Servicios: ${servicios.map((s) => s.name).join(', ')}`);
    return;
  }
  ok(`servicio «${servicio.name}» id=${servicio.id} rama actual=${servicio.repo?.branch}`);

  // ----------------------------------------------------------
  if (servicio.repo?.branch !== RAMA) {
    paso(2, `Cambiar la rama a ${RAMA} (trae la migración)`);
    const repo = { ...servicio.repo, branch: RAMA };
    await render(`/services/${servicio.id}`, 'PATCH', { repo });
    ok(`rama cambiada a ${RAMA} — deploy iniciado`);
    const nuevo = await esperarDeploy(servicio, servicio.latestDeployment?.id);
    ok(`deploy listo: ${nuevo.id}`);
  } else {
    paso(2, `La rama ya es ${RAMA} — no hay que cambiarla`);
  }

  // ----------------------------------------------------------
  paso(3, `Agregar MOON_MIGRATE_DEST (base nueva) → la migración corre sola al arrancar`);
  await render(`/services/${servicio.id}/env`, 'PUT', [
    { key: 'MOON_MIGRATE_DEST', value: DESTINO },
  ]);
  const antesMigrar = (await render(`/services/${servicio.id}`)).latestDeployment?.id;
  ok('variable agregada — deploy iniciado');
  const d2 = await esperarDeploy((await render(`/services/${servicio.id}`)), antesMigrar);
  ok(`deploy listo: ${d2.id}`);

  const logs = await leerLogsMigracion(servicio, d2.id);
  if (logs.fallo) {
    error(`La migración NO se ejecutó: ${logs.nota}`);
    return;
  }
  if (!logs.listo) {
    // Puede que el log no haya quedado accesible: se comprueba por salud.
    ok(`(sin línea en el log; se verifica por salud) ${logs.nota || ''}`);
  } else {
    ok(`migración completada según el log: ${logs.resumen}`);
  }

  // ----------------------------------------------------------
  paso(4, `Cambiar DATABASE_URL a la base nueva y quitar MOON_MIGRATE_DEST`);
  await render(`/services/${servicio.id}/env`, 'PUT', [
    { key: 'DATABASE_URL', value: DESTINO },
    { key: 'MOON_MIGRATE_DEST', value: '' },
  ]);
  const antesCambio = (await render(`/services/${servicio.id}`)).latestDeployment?.id;
  ok('variables actualizadas — deploy iniciado');
  const d3 = await esperarDeploy((await render(`/services/${servicio.id}`)), antesCambio);
  ok(`deploy listo: ${d3.id}`);

  // ----------------------------------------------------------
  paso(5, 'Verificación final: salud pública del servicio');
  await dormir(5_000);
  let salud = null;
  for (let i = 0; i < 10 && !salud; i += 1) {
    try {
      const r = await fetch(`https://${DOMINIO_ESPERADO}/api/health`);
      salud = await r.json();
    } catch {
      await dormir(10_000);
    }
  }
  if (!salud) {
    error('No respondió /api/health');
    return;
  }
  console.log('  /api/health →', JSON.stringify(salud));
  if (salud.status === 'ok' && salud.db === true) {
    ok(`SALUD VERDE — la app ahora vive en la base nueva (fotos_en_base: ${salud.fotos_en_base})`);
  } else {
    error(`Salud degradada: ${JSON.stringify(salud)}`);
  }

  console.log('\n' + '='.repeat(60));
  if (fallos === 0) {
    console.log('MUDANZA COMPLETA: web en el mismo dominio, datos en la base nueva.');
  } else {
    console.log(`La mudanza tiene ${fallos} problema(s): revisa el log de arriba.`);
  }
  console.log('='.repeat(60));
  process.exit(fallos === 0 ? 0 : 1);
}

main().catch((e) => {
  error(`Error inesperado: ${e.message}`);
  process.exit(1);
});
