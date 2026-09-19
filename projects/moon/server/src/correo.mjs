// Moon — Envío de correo
// ============================================================
// Dos caminos, el que esté configurado:
//
//   1. Resend (recomendado, gratis): basta con pegar la clave en la variable
//      RESEND_API_KEY en Render. No hay que tocar nada más.
//   2. SMTP (cualquier proveedor): SMTP_HOST, SMTP_PORT, SMTP_USER,
//      SMTP_PASS, SMTP_FROM.
//
// Si no hay ninguno, Moon no rompe nada: informa que no se pudo enviar y
// devuelve el código o el enlace en la respuesta para poder continuar.

import nodemailer from 'nodemailer';

let transporte = null;
let smtpListo = false;
let smtpRevisado = false;

export function correoConfigurado() {
  return !!process.env.RESEND_API_KEY || !!(process.env.SMTP_HOST && (process.env.SMTP_FROM || process.env.SMTP_USER));
}

export function viaDeCorreo() {
  if (process.env.RESEND_API_KEY) return 'resend';
  if (process.env.SMTP_HOST) return 'smtp';
  return '';
}

function obtenerTransporte() {
  if (smtpRevisado) return smtpListo ? transporte : null;
  smtpRevisado = true;
  const host = process.env.SMTP_HOST || '';
  const puerto = Number(process.env.SMTP_PORT || 587);
  const usuario = process.env.SMTP_USER || '';
  const clave = process.env.SMTP_PASS || '';
  const remitente = process.env.SMTP_FROM || usuario || '';

  if (!host || !remitente) { smtpListo = false; return null; }
  transporte = nodemailer.createTransport({
    host, port: puerto, secure: puerto === 465,
    auth: usuario ? { user: usuario, pass: clave } : undefined,
  });
  smtpListo = true;
  return transporte;
}

function remitentePorDefecto() {
  return process.env.MAIL_FROM
    || process.env.SMTP_FROM
    || process.env.SMTP_USER
    || 'Moon <onboarding@resend.dev>';
}

/** Envía por la API de Resend (una sola clave, sin configurar servidor). */
async function enviarConResend(destino, asunto, cuerpoTexto, adjunto) {
  const cuerpo = {
    from: remitentePorDefecto(),
    to: [destino],
    subject: asunto,
    text: cuerpoTexto,
  };
  if (adjunto) {
    cuerpo.attachments = [{
      filename: adjunto.nombre,
      content: adjunto.contenido.toString('base64'),
    }];
  }
  const res = await fetch('https://api.resend.com/emails', {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${process.env.RESEND_API_KEY}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(cuerpo),
  });
  if (!res.ok) {
    const detalle = await res.text().catch(() => '');
    throw new Error(`Resend respondió ${res.status}: ${detalle.slice(0, 200)}`);
  }
  return true;
}

/**
 * Envía un correo. `adjunto` es opcional: { nombre, contenido (Buffer) }.
 * Nunca lanza: devuelve { enviado: true|false }.
 */
/** Envía por la API de Brevo (gratis: 300/día; remitente verificado por correo). */
async function enviarConBrevo(destino, asunto, cuerpoTexto) {
  const desde = process.env.MAIL_FROM || process.env.BREVO_FROM || 'Moon <no-responder@brevo.com>';
  const [nombre, correoFrom] = desde.includes('<')
    ? [desde.split('<')[0].trim(), desde.split('<')[1].replace('>', '')]
    : ['Moon', desde];
  const res = await fetch('https://api.brevo.com/v3/smtp/email', {
    method: 'POST',
    headers: { 'api-key': process.env.BREVO_API_KEY, 'Content-Type': 'application/json', accept: 'application/json' },
    body: JSON.stringify({
      sender: { name: nombre || 'Moon', email: correoFrom },
      to: [{ email: destino }],
      subject: asunto,
      textContent: cuerpoTexto,
    }),
  });
  if (!res.ok) {
    const detalle = await res.text().catch(() => '');
    throw new Error(`Brevo respondió ${res.status}: ${detalle.slice(0, 200)}`);
  }
  return true;
}

/** Intenta enviar por el proveedor configurado. Nunca lanza. */
async function intentarEnvio(destino, asunto, cuerpoTexto, adjunto) {
  if (process.env.BREVO_API_KEY && !adjunto) {
    try {
      await enviarConBrevo(destino, asunto, cuerpoTexto);
      return { enviado: true, via: 'brevo' };
    } catch (e) {
      console.error('[correo] Brevo falló:', e.message);
    }
  }
  if (process.env.RESEND_API_KEY) {
    try {
      await enviarConResend(destino, asunto, cuerpoTexto, adjunto);
      return { enviado: true, via: 'resend' };
    } catch (e) {
      console.error('[correo] Resend falló:', e.message);
      // Resend en plan gratis solo entrega al dueño de la cuenta; si hay SMTP
      // configurado, se intenta como respaldo antes de rendirse.
      const t = obtenerTransporte();
      if (t) {
        try {
          const mensaje = { from: remitentePorDefecto(), to: destino, subject: asunto, text: cuerpoTexto };
          if (adjunto) mensaje.attachments = [{ filename: adjunto.nombre, content: adjunto.contenido }];
          await t.sendMail(mensaje);
          return { enviado: true, via: 'smtp' };
        } catch (e2) {
          console.error('[correo] SMTP de respaldo también falló:', e2.message);
          return { enviado: false, error: e2.message };
        }
      }
      return { enviado: false, error: e.message };
    }
  }

  const t = obtenerTransporte();
  if (!t) {
    console.log(`[correo] sin correo configurado; no se envía a ${destino}: ${asunto}`);
    return { enviado: false };
  }
  try {
    const mensaje = { from: remitentePorDefecto(), to: destino, subject: asunto, text: cuerpoTexto };
    if (adjunto) mensaje.attachments = [{ filename: adjunto.nombre, content: adjunto.contenido }];
    await t.sendMail(mensaje);
    return { enviado: true, via: 'smtp' };
  } catch (e) {
    console.error('[correo] fallo al enviar:', e.message);
    return { enviado: false, error: e.message };
  }
}

/**
 * Envía un correo. `adjunto` es opcional: { nombre, contenido (Buffer) }.
 * Nunca lanza: devuelve { enviado: true|false, redirigido? }.
 */
export async function enviarCorreo(destino, asunto, cuerpoTexto, { adjunto = null } = {}) {
  if (!destino) return { enviado: false, error: 'sin destinatario' };
  const resultado = await intentarEnvio(destino, asunto, cuerpoTexto, adjunto);
  if (resultado.enviado) return resultado;

  // Los proveedores gratis (Resend sin dominio propio) solo entregan al correo
  // dueño de la cuenta. Si MOON_MAIL_FALLBACK está definida, el correo se
  // reenvía ahí con una nota: la app sigue funcionando sin pagar nada.
  const respaldo = (process.env.MOON_MAIL_FALLBACK || '').trim();
  if (respaldo && respaldo.toLowerCase() !== destino.toLowerCase()) {
    const nota =
      `AVISO: este correo era para ${destino}, pero el proveedor gratuito no permite enviar a esa direccion. Lo recibes tu en su lugar.\n\n`;
    const r2 = await intentarEnvio(respaldo, asunto, nota + cuerpoTexto, adjunto);
    if (r2.enviado) return { ...r2, redirigido: destino };
  }
  return resultado;
}
