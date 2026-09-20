// Moon — Cliente de tiempo real (WebSocket)
// ============================================================
// Reconexión exponencial con latido, cola de eventos mientras se
// re-conecta y despacho por tipo. Protocolo: JSON {type, ...}.

import { wsUrl, getAccessToken, getRefreshToken, saveTokens, getUser } from './api.js';

class Realtime {
  constructor() {
    this.socket = null;
    this.listeners = new Set();
    this.connected = false;
    this.reconnectDelay = 1000;
    this.pingTimer = null;
    this.pending = [];
    this.shouldRun = false;
    this._connect = this._connect.bind(this);
  }

  // Registra un listener que recibe {type, data...}
  on(fn) { this.listeners.add(fn); return () => this.listeners.delete(fn); }

  _emit(ev) {
    for (const fn of this.listeners) {
      try { fn(ev); } catch (e) { console.error('listener error', e); }
    }
  }

  start() {
    if (this.shouldRun) return;
    this.shouldRun = true;
    // Cuando el teléfono recupera la señal, o la persona vuelve a la app, se
    // reintenta de una: sin esto, se podía tardar hasta 30 s.
    if (typeof window !== 'undefined' && !this.escuchando) {
      this.escuchando = true;
      window.addEventListener('online', () => this.despertar());
      document.addEventListener('visibilitychange', () => {
        if (document.visibilityState === 'visible') this.despertar();
      });
    }
    this._connect();
  }

  /** Reintenta la conexión ahora mismo (sin esperar el turno del reloj). */
  despertar() {
    if (!this.shouldRun || this.connected) return;
    this.reconnectDelay = 1000;
    if (this.timerReconexion) { clearTimeout(this.timerReconexion); this.timerReconexion = null; }
    if (this.timerDespertar) clearTimeout(this.timerDespertar);
    this.timerDespertar = setTimeout(() => {
      this.timerDespertar = null;
      this._connect();
    }, 250);
  }

  stop() {
    this.shouldRun = false;
    if (this.pingTimer) clearInterval(this.pingTimer);
    if (this.socket) { this.socket.onclose = null; this.socket.close(); this.socket = null; }
    this.connected = false;
  }

  _connect() {
    if (!this.shouldRun) return;
    if (this.timerReconexion) { clearTimeout(this.timerReconexion); this.timerReconexion = null; }
    // Si ya hay un tubo abierto o abriéndose, no se abre otro.
    if (this.socket && (this.socket.readyState === WebSocket.OPEN || this.socket.readyState === WebSocket.CONNECTING)) return;
    if (!getAccessToken()) {
      // Sin sesión: reintenta cuando haya token (el login llama start()).
      this.connected = false;
      return;
    }
    try {
      const token = encodeURIComponent(getAccessToken());
      const socket = new WebSocket(`${wsUrl()}?token=${token}`);
      this.socket = socket;

      socket.onopen = () => {
        this.connected = true;
        this.reconnectDelay = 1000;
        this._emit({ type: 'realtime_connected' });
        // Drenar cola
        const queue = this.pending.splice(0);
        for (const msg of queue) this.send(msg);
        // Latido cada 25 s (el servidor responde pong)
        if (this.pingTimer) clearInterval(this.pingTimer);
        this.pingTimer = setInterval(() => this.send({ type: 'ping' }), 25000);
      };

      socket.onmessage = (e) => {
        try {
          const ev = JSON.parse(e.data);
          if (ev.type === 'connected') this._emit({ type: 'realtime_ready', ...ev });
          else this._emit(ev);
        } catch { /* ignorar frames inválidos */ }
      };

      socket.onclose = () => {
        this.connected = false;
        if (this.pingTimer) { clearInterval(this.pingTimer); this.pingTimer = null; }
        this._emit({ type: 'realtime_disconnected' });
        if (this.shouldRun) {
          // Reconexión exponencial (máx 30 s)
          if (this.timerReconexion) clearTimeout(this.timerReconexion);
          this.timerReconexion = setTimeout(this._connect, this.reconnectDelay);
          this.reconnectDelay = Math.min(this.reconnectDelay * 2, 30000);
        }
      };

      socket.onerror = () => { /* onclose maneja la reconexión */ };
    } catch {
      this.connected = false;
      if (this.shouldRun && !this.timerReconexion) {
        this.timerReconexion = setTimeout(this._connect, this.reconnectDelay);
      }
    }
  }

  send(msg) {
    if (this.socket && this.socket.readyState === WebSocket.OPEN) {
      this.socket.send(JSON.stringify(msg));
      return true;
    }
    // Cola limitada para no acumular indefinidamente
    if (this.pending.length < 50) this.pending.push(msg);
    return false;
  }
}

export const realtime = new Realtime();
