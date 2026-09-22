// Moon — Motor de música para historias (Web Audio API)
// ============================================================
// Proporciona vibras musicales nativas para ambientar las historias
// con síntesis de audio armónica y ligera en el navegador.

export const PISTAS_MUSICA = [
  { id: 'lofi', titulo: 'Moonlight Lo-Fi', autor: 'Luna Sound', emoji: '🌙', tempo: 80, tipo: 'chill' },
  { id: 'synth', titulo: 'Cyber Synthwave', autor: 'Neon Grid', emoji: '⚡', tempo: 110, tipo: 'retro' },
  { id: 'sunset', titulo: 'Sunset Chill', autor: 'Solar Waves', emoji: '🌅', tempo: 88, tipo: 'acustico' },
  { id: 'cosmic', titulo: 'Cosmic Dreams', autor: 'Astro Sphere', emoji: '🌌', tempo: 70, tipo: 'ambient' },
  { id: 'urban', titulo: 'Urban Beats', autor: 'Orbit Club', emoji: '🔥', tempo: 100, tipo: 'beat' },
  { id: 'pop', titulo: 'Astro Pop', autor: 'Starlight', emoji: '🪐', tempo: 118, tipo: 'pop' },
];

let audioCtx = null;
let timerId = null;
let activoId = null;

function obtenerContexto() {
  if (!audioCtx) {
    const AudioContextClass = window.AudioContext || window.webkitAudioContext;
    if (AudioContextClass) {
      audioCtx = new AudioContextClass();
    }
  }
  if (audioCtx && audioCtx.state === 'suspended') {
    audioCtx.resume();
  }
  return audioCtx;
}

// Frecuencias de notas musicales (Escala Pentatónica y acordes agradables)
const NOTAS = {
  C3: 130.81, E3: 164.81, G3: 196.00, B3: 246.94,
  C4: 261.63, D4: 293.66, E4: 329.63, G4: 392.00, A4: 440.00, B4: 493.88,
  C5: 523.25, D5: 587.33, E5: 659.25, G5: 783.99, A5: 880.00,
};

const PATRONES = {
  lofi: [
    [NOTAS.C4, NOTAS.E4, NOTAS.G4],
    [NOTAS.A4, NOTAS.C5, NOTAS.E5],
    [NOTAS.D4, NOTAS.G4, NOTAS.B4],
    [NOTAS.C4, NOTAS.G4, NOTAS.E5],
  ],
  synth: [
    [NOTAS.C3, NOTAS.G4],
    [NOTAS.E3, NOTAS.B4],
    [NOTAS.A4, NOTAS.E5],
    [NOTAS.G3, NOTAS.D5],
  ],
  sunset: [
    [NOTAS.E4, NOTAS.G4, NOTAS.B4],
    [NOTAS.C4, NOTAS.E4, NOTAS.A4],
    [NOTAS.D4, NOTAS.G4, NOTAS.B4],
    [NOTAS.C4, NOTAS.G4, NOTAS.C5],
  ],
  cosmic: [
    [NOTAS.C3, NOTAS.G3, NOTAS.D4],
    [NOTAS.A3, NOTAS.E4, NOTAS.B4],
    [NOTAS.F3, NOTAS.C4, NOTAS.G4],
    [NOTAS.G3, NOTAS.D4, NOTAS.A4],
  ],
  urban: [
    [NOTAS.C4, NOTAS.E4],
    [NOTAS.G4, NOTAS.A4],
    [NOTAS.E4, NOTAS.D4],
    [NOTAS.B3, NOTAS.G4],
  ],
  pop: [
    [NOTAS.C4, NOTAS.G4, NOTAS.C5],
    [NOTAS.G4, NOTAS.B4, NOTAS.D5],
    [NOTAS.A4, NOTAS.C5, NOTAS.E5],
    [NOTAS.F4, NOTAS.A4, NOTAS.C5],
  ],
};

function tocarAcorde(ctx, frecuencias, duracion, tipoOsc = 'sine') {
  frecuencias.forEach((freq) => {
    try {
      const osc = ctx.createOscillator();
      const gain = ctx.createGain();
      const filter = ctx.createBiquadFilter();

      filter.type = 'lowpass';
      filter.frequency.setValueAtTime(tipoOsc === 'sawtooth' ? 1200 : 800, ctx.currentTime);

      osc.type = tipoOsc;
      osc.frequency.setValueAtTime(freq, ctx.currentTime);

      gain.gain.setValueAtTime(0.001, ctx.currentTime);
      gain.gain.exponentialRampToValueAtTime(0.12, ctx.currentTime + 0.15);
      gain.gain.exponentialRampToValueAtTime(0.0001, ctx.currentTime + duracion);

      osc.connect(filter);
      filter.connect(gain);
      gain.connect(ctx.destination);

      osc.start();
      osc.stop(ctx.currentTime + duracion + 0.05);
    } catch (_) {}
  });
}

export function reproducirMusica(pistaId) {
  detenerMusica();
  const ctx = obtenerContexto();
  if (!ctx) return;

  const pista = PISTAS_MUSICA.find((p) => p.id === pistaId) || PISTAS_MUSICA[0];
  const acordes = PATRONES[pista.id] || PATRONES.lofi;
  const intervaloMs = Math.round((60000 / pista.tempo) * 2);
  const tipoOsc = pista.tipo === 'retro' ? 'sawtooth' : pista.tipo === 'beat' ? 'triangle' : 'sine';

  let paso = 0;
  activoId = pista.id;

  const ejecutar = () => {
    if (activoId !== pista.id) return;
    const acorde = acordes[paso % acordes.length];
    tocarAcorde(ctx, acorde, intervaloMs / 1000 * 1.5, tipoOsc);
    paso++;
    timerId = setTimeout(ejecutar, intervaloMs);
  };

  ejecutar();
}

export function detenerMusica() {
  activoId = null;
  if (timerId) {
    clearTimeout(timerId);
    timerId = null;
  }
}
