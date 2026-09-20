// Moon — Llamadas de voz y video dentro del chat
// ============================================================
// La voz NO pasa por el servidor: viaja directa entre los dos teléfonos
// (así funcionan las llamadas de Facebook). El servidor solo hace de
// cartero: avisa que alguien llama, pasa el «acepto», el «cuelgo» y los
// datos de conexión. Ese cartero vive en el mismo tubo del chat (WebSocket).
//
// Aquí vive todo: el estado de la llamada, la pantalla completa de llamada
// (entrante, saliente y en curso), el timbre (sin archivos: notas suaves) y
// el registro de la llamada en el chat al colgar.

import React, {
  createContext, useCallback, useContext, useEffect, useRef, useState,
} from 'react';
import { createPortal } from 'react-dom';
import { realtime } from './realtime.js';
import { api } from './api.js';
import { toast } from './ui.js';
import { leer as leerPref } from './prefs.js';
import Avatar from './components/Avatar.jsx';
import {
  IconPhone, IconPhoneOff, IconMic, IconMicOff, IconVideoLlamada, IconCameraOff,
} from './components/Icons.jsx';

const LlamadasContext = createContext(null);

// Cuánto suena antes de darse por no contestada (35 s, como las demás apps).
const ESPERA_TIMBRADO_MS = 35000;
// Cuando el otro no tiene Moon abierto, se le avisa al teléfono: se le da más
// tiempo para abrirlo y contestar (el servidor guarda la llamada 90 s).
const ESPERA_AVISO_MS = 60000;

function identificador() {
  return `L${Date.now().toString(36)}${Math.random().toString(36).slice(2, 8)}`;
}

function reloj(ms) {
  const total = Math.max(0, Math.floor(ms / 1000));
  const m = Math.floor(total / 60);
  const s = total % 60;
  return `${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`;
}

// ---------- Timbre (sin archivos de sonido) ----------
// El navegador solo deja sonar si la persona ya tocó la pantalla alguna vez:
// se prepara un «altavoz» al primer toque y el timbre lo reutiliza.
let altavoz = null;
function desbloquearSonido() {
  try {
    const AC = window.AudioContext || window.webkitAudioContext;
    if (!AC) return;
    if (!altavoz) altavoz = new AC();
    if (altavoz.state === 'suspended') altavoz.resume().catch(() => {});
  } catch { /* sin sonido */ }
}

// Dos notas suaves que se repiten. El de entrada es más «llamativo» y el de
// salida, más bajito, como cuando esperas a que contesten.
function crearTimbre(tipo) {
  // Si la persona apago los sonidos en Ajustes, solo vibra.
  if (leerPref('sonido') !== 'si') return { parar() {} };
  const AC = window.AudioContext || window.webkitAudioContext;
  if (!AC) return { parar() {} };
  let ctx;
  try {
    if (altavoz && altavoz.state === 'closed') altavoz = null;
    ctx = altavoz || new AC();
  } catch { return { parar() {} }; }
  const compartido = ctx === altavoz;
  if (ctx.state === 'suspended') ctx.resume().catch(() => {});
  const notas = tipo === 'entrante' ? [660, 880, 660, 880] : [440, 480];
  const volumen = tipo === 'entrante' ? 0.09 : 0.045;
  let vivo = true;

  function vuelta() {
    if (!vivo) return;
    const t0 = ctx.currentTime;
    notas.forEach((f, i) => {
      const osc = ctx.createOscillator();
      const gain = ctx.createGain();
      osc.type = 'sine';
      osc.frequency.value = f;
      const inicio = t0 + i * 0.28;
      gain.gain.setValueAtTime(0.0001, inicio);
      gain.gain.exponentialRampToValueAtTime(volumen, inicio + 0.03);
      gain.gain.exponentialRampToValueAtTime(0.0001, inicio + 0.26);
      osc.connect(gain).connect(ctx.destination);
      osc.start(inicio);
      osc.stop(inicio + 0.3);
    });
  }

  vuelta();
  const cada = tipo === 'entrante' ? 2600 : 2200;
  const timer = setInterval(vuelta, cada);
  return {
    parar() {
      vivo = false;
      clearInterval(timer);
      // El altavoz compartido sigue vivo: se reutiliza en la próxima llamada.
      if (!compartido) {
        try { ctx.close(); } catch { /* ya cerrado */ }
      }
    },
  };
}

export function useLlamadas() {
  return useContext(LlamadasContext);
}

export function LlamadasProvider({ children }) {
  const [estado, setEstado] = useState('inactiva'); // inactiva|saliendo|entrando|activa|terminada
  const [partner, setPartner] = useState(null);
  const [tipo, setTipo] = useState('voz');
  const [conexion, setConexion] = useState(''); // preparando|conectando|conectada|fallando
  const [microApagado, setMicroApagado] = useState(false);
  const [camaraApagada, setCamaraApagada] = useState(false);
  const [detalle, setDetalle] = useState('');
  const [segundos, setSegundos] = useState(0);
  const [videoRemoto, setVideoRemoto] = useState(null);
  const [videoLocal, setVideoLocal] = useState(null);

  const pc = useRef(null);
  const local = useRef(null);
  const llamada = useRef(null); // { id, partner, tipo, conversacion }
  const candidatosPendientes = useRef([]);
  const timbre = useRef(null);
  const cronometro = useRef(null);
  const espera = useRef(null);
  const anotadas = useRef(new Set());  // llamadas ya anotadas en el chat
  const vivo = useRef(true);
  // Lo que llega antes de tiempo (la otra parte contesta muy rápido) se guarda
  // y se usa en cuanto la llamada está lista.
  const pendientes = useRef([]);

  useEffect(() => () => { vivo.current = false; }, []);

  const pararTimbre = useCallback(() => {
    if (timbre.current) { timbre.current.parar(); timbre.current = null; }
    if (navigator.vibrate) { try { navigator.vibrate(0); } catch { /* sin vibrador */ } }
  }, []);

  const limpiar = useCallback(() => {
    pararTimbre();
    if (cronometro.current) { clearInterval(cronometro.current); cronometro.current = null; }
    if (espera.current) { clearTimeout(espera.current); espera.current = null; }
    if (pc.current) {
      try { pc.current.onicecandidate = null; pc.current.ontrack = null; pc.current.close(); } catch { /* ya cerrada */ }
      pc.current = null;
    }
    if (local.current) {
      for (const pista of local.current.getTracks()) { try { pista.stop(); } catch { /* ya detenida */ } }
      local.current = null;
    }
    candidatosPendientes.current = [];
    pendientes.current = [];
    setVideoRemoto(null);
    setVideoLocal(null);
  }, [pararTimbre]);

  const terminar = useCallback((aviso) => {
    const datos = llamada.current;
    const segundines = datos?.segundos || 0;
    limpiar();
    llamada.current = null;
    setEstado('inactiva');
    setConexion('');
    setMicroApagado(false);
    setCamaraApagada(false);
    setSegundos(0);
    if (aviso) toast.info(aviso);
    void segundines;
  }, [limpiar]);

  // ---------- Anotar la llamada en el chat ----------
  // Queda como una fila más del hilo: «Llamada de voz · 02:14» o «Llamada
  // perdida». La escribe UNA vez quien empezó la llamada, así no salen dos
  // filas iguales; el otro lado la ve llegar por el tubo en vivo.
  const anotar = useCallback(async (datos, clase, duracionMs, quien) => {
    if (!datos?.conversacion || !datos.soyQuienLlama) return;
    // Una sola fila por llamada, aunque lleguen dos avisos a la vez.
    if (datos.id) {
      if (anotadas.current.has(datos.id)) return;
      if (anotadas.current.size > 50) anotadas.current.clear();
      anotadas.current.add(datos.id);
    }
    try {
      const creada = await api.post(`/api/messages/conversations/${datos.conversacion}/messages`, {
        kind: clase,
        duracion_ms: Math.max(0, Math.round(duracionMs)),
        reply_to_id: quien?.id || undefined,
      });
      // Quien hizo la llamada no recibe su propio mensaje por el tubo: se le
      // avisa aqui para que la fila salga en su hilo en el momento.
      if (creada?.id) {
        window.dispatchEvent(new CustomEvent('moon:mensaje-propio', {
          detail: { conversation_id: Number(datos.conversacion), message: creada },
        }));
      }
    } catch { /* si falla el registro, la llamada no se rompe */ }
  }, []);

  // ---------- La conexión entre los dos teléfonos ----------
  const configurarConexion = useCallback(async (esQuienLlama) => {
    const datos = llamada.current;
    if (!datos) return null;
    let ice = [{ urls: ['stun:stun.l.google.com:19302'] }];
    try {
      const cfg = await api.get('/api/llamadas/config');
      if (cfg?.iceServers?.length) ice = cfg.iceServers;
    } catch { /* sin configuración: se intenta directo */ }

    const conexionPc = new RTCPeerConnection({ iceServers: ice });
    pc.current = conexionPc;

    conexionPc.onicecandidate = (e) => {
      if (!e.candidate) return;
      realtime.send({
        type: 'call_signal',
        call_id: datos.id,
        to: datos.partner.id,
        sobre: { candidato: e.candidate.toJSON() },
      });
    };
    conexionPc.ontrack = (e) => {
      const [flujo] = e.streams;
      setVideoRemoto(flujo || null);
    };
    conexionPc.onconnectionstatechange = () => {
      const st = conexionPc.connectionState;
      if (st === 'connected') setConexion('conectada');
      else if (st === 'connecting' || st === 'new') setConexion((v) => (v === 'conectada' ? v : 'conectando'));
      else if (st === 'failed') {
        setConexion('fallando');
        setDetalle('Se cortó la conexión');
      } else if (st === 'disconnected') setConexion('conectando');
    };

    // El micrófono y la cámara se piden aquí (solo cuando ya hay llamada).
    const pistas = [];
    try {
      const flujo = await navigator.mediaDevices.getUserMedia({
        audio: true,
        video: datos.tipo === 'video' ? { facingMode: 'user', width: { ideal: 1280 } } : false,
      });
      local.current = flujo;
      setVideoLocal(flujo);
      for (const pista of flujo.getTracks()) {
        pistas.push(pista);
        conexionPc.addTrack(pista, flujo);
      }
    } catch (e) {
      setDetalle('No se pudo abrir el micrófono o la cámara');
      toast.err('Da permiso al micrófono y a la cámara para llamar');
      throw e;
    }
    void esQuienLlama;
    setConexion('conectando');
    return conexionPc;
  }, []);

  const aplicarPendientes = useCallback(async () => {
    const conexionPc = pc.current;
    if (!conexionPc) return;
    const lista = pendientes.current.splice(0);
    for (const sobre of lista) await aplicarSobre(sobre, conexionPc);
  }, []);

  // Aplica un sobre que llega por el tubo (oferta, respuesta o candidato).
  const aplicarSobre = useCallback(async (sobre, conexionPc = pc.current) => {
    if (!conexionPc || !sobre) return;
    try {
      if (sobre.descripcion) {
        const esOferta = sobre.descripcion.type === 'offer';
        await conexionPc.setRemoteDescription(sobre.descripcion);
        if (esOferta) {
          const respuesta = await conexionPc.createAnswer();
          await conexionPc.setLocalDescription(respuesta);
          realtime.send({
            type: 'call_signal',
            call_id: llamada.current?.id,
            to: llamada.current?.partner.id,
            sobre: { descripcion: conexionPc.localDescription.toJSON() },
          });
        } else {
          // La respuesta ya está puesta: ahora sí entran los candidatos.
          const guardados = candidatosPendientes.current.splice(0);
          for (const c of guardados) await conexionPc.addIceCandidate(c).catch(() => {});
        }
      } else if (sobre.candidato) {
        if (conexionPc.remoteDescription) {
          await conexionPc.addIceCandidate(sobre.candidato).catch(() => {});
        } else {
          candidatosPendientes.current.push(sobre.candidato);
        }
      }
    } catch { /* sobre atrasado o repetido: no rompe la llamada */ }
  }, []);

  const arrancarCronometro = useCallback(() => {
    if (cronometro.current) return;
    const inicio = Date.now();
    cronometro.current = setInterval(() => {
      const s = Math.floor((Date.now() - inicio) / 1000);
      setSegundos(s);
      if (llamada.current) llamada.current.segundos = s;
    }, 1000);
  }, []);

  // ---------- Llamar ----------
  const llamar = useCallback(async (partnerDatos, tipoLlamada = 'voz', conversacionId = null) => {
    if (!partnerDatos?.id) return;
    if (estado !== 'inactiva') { toast.info('Ya hay una llamada en curso'); return; }
    const id = identificador();
    llamada.current = {
      id, partner: partnerDatos, tipo: tipoLlamada, conversacion: conversacionId, segundos: 0,
      soyQuienLlama: true,
    };
    setPartner(partnerDatos);
    setTipo(tipoLlamada);
    setDetalle('');
    setSegundos(0);
    setConexion('conectando');
    setEstado('saliendo');
    realtime.send({
      type: 'call_start',
      call_id: id,
      to: Number(partnerDatos.id),
      tipo: tipoLlamada,
      conversation_id: conversacionId,
    });
    timbre.current = crearTimbre('saliendo');
    espera.current = setTimeout(() => {
      realtime.send({ type: 'call_end', call_id: id, to: Number(partnerDatos.id), segundos: 0 });
      anotar({ id, conversacion: conversacionId, soyQuienLlama: true }, 'llamada_perdida', 0);
      terminar('No contestó');
    }, ESPERA_TIMBRADO_MS);
  }, [estado, anotar, terminar]);

  // ---------- Contestar ----------
  const contestar = useCallback(async () => {
    const datos = llamada.current;
    if (!datos) return;
    pararTimbre();
    setEstado('activa');
    try {
      await configurarConexion(false);
    } catch {
      realtime.send({ type: 'call_reject', call_id: datos.id, to: datos.partner.id, motivo: 'sin_permiso' });
      terminar();
      return;
    }
    realtime.send({ type: 'call_accept', call_id: datos.id, to: datos.partner.id });
    await aplicarPendientes();
    arrancarCronometro();
  }, [configurarConexion, pararTimbre, arrancarCronometro, terminar, aplicarPendientes]);

  // ---------- Rechazar / colgar ----------
  const colgar = useCallback((motivo = 'colgo') => {
    const datos = llamada.current;
    if (!datos) { terminar(); return; }
    pararTimbre();
    const hablado = datos.segundos || 0;
    realtime.send({ type: 'call_end', call_id: datos.id, to: datos.partner.id, segundos: hablado });
    // Queda anotado en el chat: con duración si se habló (aunque fuera un
    // segundo), o como llamada perdida si nadie contestó.
    if (estado === 'activa' || estado === 'saliendo') {
      const clase = (estado === 'activa' || hablado > 0)
        ? (datos.tipo === 'video' ? 'llamada_video' : 'llamada_voz')
        : 'llamada_perdida';
      anotar(datos, clase, hablado * 1000);
    }
    void motivo;
    terminar();
  }, [anotar, pararTimbre, terminar, estado]);

  const rechazar = useCallback(() => {
    const datos = llamada.current;
    pararTimbre();
    if (datos) {
      realtime.send({ type: 'call_reject', call_id: datos.id, to: datos.partner.id, motivo: 'rechazada' });
      anotar(datos, 'llamada_perdida', 0);
    }
    terminar();
  }, [anotar, pararTimbre, terminar]);

  // ---------- Interruptores ----------
  const alternarMicro = useCallback(() => {
    const flujo = local.current;
    if (!flujo) return;
    const pistas = flujo.getAudioTracks();
    const apagar = pistas.some((p) => p.enabled);
    for (const p of pistas) p.enabled = !apagar;
    setMicroApagado(apagar);
  }, []);

  const alternarCamara = useCallback(() => {
    const flujo = local.current;
    if (!flujo) return;
    const pistas = flujo.getVideoTracks();
    if (pistas.length === 0) return;
    const apagar = pistas.some((p) => p.enabled);
    for (const p of pistas) p.enabled = !apagar;
    setCamaraApagada(apagar);
  }, []);

  // ---------- Lo que llega por el tubo ----------
  useEffect(() => {
    const quitar = realtime.on((ev) => {
      if (!ev || typeof ev.type !== 'string') return;
      // El tubo volvió: si la llamada sigue «saliendo», se repite el aviso por
      // si el primero se perdió en el parpadeo de la conexión.
      if (ev.type === 'realtime_connected') {
        const enCurso = llamada.current;
        if (enCurso?.soyQuienLlama && (estado === 'saliendo' || estado === 'activa')) {
          realtime.send({
            type: 'call_start',
            call_id: enCurso.id,
            to: Number(enCurso.partner.id),
            tipo: enCurso.tipo,
            conversation_id: enCurso.conversacion,
          });
        }
        return;
      }
      if (!ev.type.startsWith('call_')) return;
      const datos = llamada.current;

      if (ev.type === 'call_ring') {
        // El mismo timbrazo repetido (pasa cuando se manda por segunda vez al
        // abrir Moon) no es otra llamada: se ignora.
        if (datos && datos.id === ev.call_id) return;
        if (estado !== 'inactiva') {
          realtime.send({ type: 'call_reject', call_id: ev.call_id, to: ev.de?.id, motivo: 'ocupado' });
          return;
        }
        llamada.current = {
          id: ev.call_id, partner: ev.de, tipo: ev.tipo, conversacion: ev.conversation_id, segundos: 0,
          soyQuienLlama: false,
        };
        setPartner(ev.de);
        setTipo(ev.tipo);
        setDetalle('');
        setConexion('');
        setSegundos(0);
        setEstado('entrando');
        timbre.current = crearTimbre('entrante');
        if (navigator.vibrate) { try { navigator.vibrate([280, 140, 280, 140, 280]); } catch { /* sin vibrador */ } }
        espera.current = setTimeout(() => {
          realtime.send({ type: 'call_reject', call_id: ev.call_id, to: ev.de?.id, motivo: 'sin_respuesta' });
          terminar('Llamada perdida');
        }, ESPERA_TIMBRADO_MS);
        return;
      }

      if (!datos) return;

      if (ev.type === 'call_aceptada') {
        pararTimbre();
        setDetalle('');
        if (espera.current) { clearTimeout(espera.current); espera.current = null; }
        (async () => {
          setEstado('activa');
          try {
            const conexionPc = await configurarConexion(true);
            const oferta = await conexionPc.createOffer();
            await conexionPc.setLocalDescription(oferta);
            realtime.send({
              type: 'call_signal',
              call_id: datos.id,
              to: datos.partner.id,
              sobre: { descripcion: conexionPc.localDescription.toJSON() },
            });
            arrancarCronometro();
            await aplicarPendientes();
          } catch {
            realtime.send({ type: 'call_end', call_id: datos.id, to: datos.partner.id, segundos: 0 });
            terminar('No se pudo completar la llamada');
          }
        })();
        return;
      }

      if (ev.type === 'call_senal') {
        if (pc.current) aplicarSobre(ev.sobre);
        else pendientes.current.push(ev.sobre);
        return;
      }

      if (ev.type === 'call_rechazada') {
        const motivos = {
          ocupado: 'Está en otra llamada',
          sin_respuesta: 'No contestó',
          sin_permiso: 'No pudo atender',
          rechazada: 'Rechazó la llamada',
          rapido: 'Espera un momento antes de volver a llamar',
          tu_llamada: 'Ya tienes una llamada en curso',
        };
        if (motivos[ev.motivo]) toast.info(motivos[ev.motivo]);
        // Llamada perdida para quien llamó (salvo si ni alcanzó a sonar).
        const perdidas = ['ocupado', 'sin_respuesta', 'sin_permiso', 'rechazada'];
        if (ev.call_id === datos.id && datos.soyQuienLlama && perdidas.includes(String(ev.motivo))) {
          anotar(datos, 'llamada_perdida', 0);
        }
        terminar();
        return;
      }

      if (ev.type === 'call_avisando') {
        // No tiene Moon abierto, pero sí teléfono: se le avisó y la llamada
        // sigue esperando a que lo abra y conteste.
        if (ev.call_id !== datos.id) return;
        setDetalle('Le está sonando el teléfono…');
        toast.info('Le está sonando el teléfono');
        if (espera.current) { clearTimeout(espera.current); espera.current = null; }
        espera.current = setTimeout(() => {
          realtime.send({ type: 'call_end', call_id: datos.id, to: datos.partner.id, segundos: 0 });
          anotar(datos, 'llamada_perdida', 0);
          terminar('No contestó');
        }, ESPERA_AVISO_MS);
        return;
      }

      if (ev.type === 'call_sin_conexion') {
        terminar('No tiene Moon abierto ni los avisos encendidos');
        return;
      }

      if (ev.type === 'call_terminada') {
        const hablado = Number(ev.segundos || 0);
        // Si la otra parte cuelga, aquí se anota lo que duró.
        if (estado === 'activa') anotar(datos, datos.tipo === 'video' ? 'llamada_video' : 'llamada_voz', hablado * 1000);
        else if (estado === 'entrando') anotar(datos, 'llamada_perdida', 0);
        terminar(ev.motivo === 'se_fue' ? 'La otra persona salió de Moon' : (estado === 'activa' ? 'Llamada terminada' : ''));
      }
    });
    return quitar;
  }, [estado, configurarConexion, aplicarSobre, aplicarPendientes, anotar, pararTimbre, arrancarCronometro, terminar]);

  // El primer toque en la pantalla deja listo el altavoz del timbre.
  useEffect(() => {
    const despertar = () => desbloquearSonido();
    window.addEventListener('pointerdown', despertar);
    window.addEventListener('touchstart', despertar);
    window.addEventListener('keydown', despertar);
    return () => {
      window.removeEventListener('pointerdown', despertar);
      window.removeEventListener('touchstart', despertar);
      window.removeEventListener('keydown', despertar);
    };
  }, []);

  // Marca de tiempo del cronómetro al cerrar la pestaña.
  useEffect(() => {
    function alSalir() {
      const datos = llamada.current;
      if (datos && (estado === 'activa' || estado === 'saliendo' || estado === 'entrando')) {
        realtime.send({ type: 'call_end', call_id: datos.id, to: datos.partner.id, segundos: datos.segundos || 0 });
      }
    }
    window.addEventListener('pagehide', alSalir);
    return () => window.removeEventListener('pagehide', alSalir);
  }, [estado]);

  const valor = { estado, tipo, partner, llamar, colgar, contestar, rechazar, activa: estado !== 'inactiva' };

  return (
    <LlamadasContext.Provider value={valor}>
      {children}
      {estado !== 'inactiva' ? (
        <PantallaLlamada
          estado={estado}
          tipo={tipo}
          partner={partner}
          conexion={conexion}
          detalle={detalle}
          segundos={segundos}
          microApagado={microApagado}
          camaraApagada={camaraApagada}
          videoLocal={videoLocal}
          videoRemoto={videoRemoto}
          onColgar={colgar}
          onContestar={contestar}
          onRechazar={rechazar}
          onMicro={alternarMicro}
          onCamara={alternarCamara}
        />
      ) : null}
    </LlamadasContext.Provider>
  );
}

// ---------- La pantalla de la llamada (a pantalla completa) ----------
function PantallaLlamada({
  estado, tipo, partner, conexion, detalle, segundos,
  microApagado, camaraApagada, videoLocal, videoRemoto,
  onColgar, onContestar, onRechazar, onMicro, onCamara,
}) {
  const nombre = partner?.display_name || partner?.username || 'Llamada';
  // «entrando»: alguien llama · «saliendo»: estás llamando · «activa»: en curso.
  const esVideo = tipo === 'video';
  const conectada = conexion === 'conectada';
  const etiquetaConexion = conexion === 'conectada' ? 'En llamada'
    : conexion === 'fallando' ? 'Sin conexión'
      : estado === 'entrando' ? (esVideo ? 'Videollamada entrante' : 'Llamada entrante')
        : estado === 'saliendo' ? 'Llamando…' : 'Conectando…';

  const remotoRef = useRef(null);
  const audioRef = useRef(null);
  const localRef = useRef(null);

  useEffect(() => {
    if (remotoRef.current && videoRemoto) remotoRef.current.srcObject = videoRemoto;
    // En llamada de voz, la voz del otro entra por aqui (el reproductor es
    // invisible): es el equivalente a llevarte el telefono a la oreja.
    if (audioRef.current && videoRemoto) audioRef.current.srcObject = videoRemoto;
  }, [videoRemoto, estado]);

  useEffect(() => {
    if (localRef.current && videoLocal) localRef.current.srcObject = videoLocal;
  }, [videoLocal, estado]);

  return createPortal(
    <div
      className={`llamada llamada-${estado} ${esVideo ? 'es-video' : 'es-voz'} ${conectada ? 'conectada' : ''}`}
      data-llamada={estado}
      data-conexion={conexion || 'inicial'}
      data-tipo={tipo}
      role="dialog"
      aria-modal="true"
      aria-label={`Llamada con ${nombre}`}
    >
      {/* Fondo: el video del otro si es videollamada; si no, el cielo de Moon. */}
      {estado === 'activa' && esVideo ? (
        <video ref={remotoRef} className="video-remoto" autoPlay playsInline />
      ) : (
        <div className="cielo-llamada" aria-hidden="true" />
      )}
      {/* Llamada de voz: por aqui entra la voz (no se ve). */}
      {estado === 'activa' && !esVideo ? (
        <audio ref={audioRef} autoPlay playsInline />
      ) : null}

      <div className="llamada-velo" aria-hidden="true" />

      {estado === 'entrando' ? (
        <div className="llamada-quien">
          <span className="llamada-avatar con-ondas">
            <Avatar user={partner} size="lg" />
            <span className="onda" aria-hidden="true" />
            <span className="onda dos" aria-hidden="true" />
          </span>
          <b className="llamada-nombre">{nombre}</b>
          <span className="llamada-estado">{etiquetaConexion}</span>
          {!esVideo ? <span className="llamada-nota">Llamada de voz</span> : <span className="llamada-nota">Videollamada</span>}
          <div className="llamada-botones">
            <button type="button" className="boton-llamada rojo" onClick={onRechazar} aria-label="Rechazar">
              <IconPhoneOff />
              <span>Rechazar</span>
            </button>
            <button type="button" className="boton-llamada verde latir" onClick={onContestar} aria-label="Aceptar">
              {esVideo ? <IconVideoLlamada /> : <IconPhone />}
              <span>Aceptar</span>
            </button>
          </div>
        </div>
      ) : null}

      {estado === 'saliendo' ? (
        <div className="llamada-quien">
          <span className="llamada-avatar con-ondas">
            <Avatar user={partner} size="lg" />
            <span className="onda" aria-hidden="true" />
            <span className="onda dos" aria-hidden="true" />
          </span>
          <b className="llamada-nombre">{nombre}</b>
          {/* Mientras llama se muestra lo que va pasando: «Llamando…» o
              «Le está sonando el teléfono…» si no tiene Moon abierto. */}
          <span className="llamada-estado">{detalle || etiquetaConexion}</span>
          <div className="llamada-botones">
            <button type="button" className="boton-llamada rojo" onClick={() => onColgar()} aria-label="Cancelar">
              <IconPhoneOff />
              <span>Cancelar</span>
            </button>
          </div>
        </div>
      ) : null}

      {estado === 'activa' ? (
        <>
          {esVideo ? (
            <div className="llamada-cabecera">
              <b>{nombre}</b>
              <span className="llamada-reloj">{reloj(segundos * 1000)}</span>
              {!conectada ? <span className="llamada-aviso">{etiquetaConexion}</span> : null}
            </div>
          ) : (
            <div className="llamada-quien activa">
              <span className="llamada-avatar halo">
                <Avatar user={partner} size="lg" />
              </span>
              <b className="llamada-nombre">{nombre}</b>
              <span className="llamada-reloj">{reloj(segundos * 1000)}</span>
              <span className="llamada-estado">{etiquetaConexion}</span>
            </div>
          )}

          {esVideo ? (
            <div className={`video-local ${camaraApagada ? 'apagada' : ''}`}>
              <video ref={localRef} autoPlay playsInline muted />
              {camaraApagada ? <span className="video-local-tapa">Cámara apagada</span> : null}
            </div>
          ) : null}

          <div className="llamada-controles">
            <button
              type="button"
              className={`control ${microApagado ? 'apagado' : ''}`}
              onClick={onMicro}
              aria-pressed={microApagado}
              aria-label={microApagado ? 'Encender el micrófono' : 'Apagar el micrófono'}
              title={microApagado ? 'Encender el micrófono' : 'Silenciar'}
            >
              {microApagado ? <IconMicOff /> : <IconMic />}
            </button>
            {esVideo ? (
              <button
                type="button"
                className={`control ${camaraApagada ? 'apagado' : ''}`}
                onClick={onCamara}
                aria-pressed={camaraApagada}
                aria-label={camaraApagada ? 'Encender la cámara' : 'Apagar la cámara'}
                title={camaraApagada ? 'Encender la cámara' : 'Apagar la cámara'}
              >
                {camaraApagada ? <IconCameraOff /> : <IconVideoLlamada />}
              </button>
            ) : null}
            <button type="button" className="control colgar" onClick={() => onColgar()} aria-label="Colgar" title="Colgar">
              <IconPhoneOff />
            </button>
          </div>

          {detalle ? <span className="llamada-error">{detalle}</span> : null}
        </>
      ) : null}

    </div>,
    document.body,
  );
}
