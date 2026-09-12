// Moon — Envío de correo
// ============================================================
// Usa SMTP si está configurado (SMTP_HOST, SMTP_PORT, SMTP_USER, SMTP_PASS,
// SMTP_FROM). Si no lo está —por ejemplo en desarrollo— no manda nada y lo
// informa, sin romper el flujo: el código o el enlace se devuelven en la
// respuesta para poder terminar la operación.

import nodemailer from 'nodemailer';

let transporte = null;
let configurado = null;

function obtenerTransporte() {
  if (configurado !== null) return transporte;
  const host = process.env.SMTP_HOST || '';
  const puerto = Number(process.env.SMTP_PORT || 587);
  const usuario = process.env.SMTP_USER || '';
  const clave = process.env.SMTP_PASS || '';
  const remitente = process.env.SMTP_FROM || usuario || '';

  if (!host || !remitente) {
    configurado = false;
    transporte = null;
    return transporte;
  }
  transporte = nodemailer.createTransport({
    host,
    port: puerto,
    secure: puerto === 465,
    auth: usuario ? { user: usuario, pass: clave } : undefined,
  });
  configurado = true;
  return transporte;
}

export async function enviarCorreo(destino, asunto, cuerpoTexto) {
  const t = obtenerTransporte();
  const remitente = process.env.SMTP_FROM || process.env.SMTP_USER || 'moon@localhost';
  if (!t) {
    console.log(`[correo] sin SMTP configurado; no se envía a ${destino}: ${asunto}`);
    return { enviado: false };
  }
  try {
    await t.sendMail({ from: remitente, to: destino, subject: asunto, text: cuerpoTexto });
    return { enviado: true };
  } catch (e) {
    console.error('[correo] fallo al enviar:', e.message);
    return { enviado: false, error: e.message };
  }
}
