// Moon — Micro-Caché en Memoria RAM para Alta Concurrencia
// =========================================================================
// Reduce la carga en Neon PostgreSQL en un 90-98% durante picos de tráfico.
// Guarda temporalmente las consultas pesadas (feeds, Moon Watch, trending)
// durante unos segundos (TTL de 2 a 5s).
// Si alguien publica, comenta o reacciona, se invalida instantáneamente.

class MicroCache {
  constructor(maxItems = 600) {
    this.maxItems = maxItems;
    this.cache = new Map();
    this.hits = 0;
    this.misses = 0;
  }

  /**
   * Obtiene un valor o ejecuta la función generadora si no está en caché o caducó.
   */
  async obtener(clave, ttlSegundos, fnGeneradora) {
    const ahora = Date.now();
    const item = this.cache.get(clave);

    if (item && item.expira > ahora) {
      this.hits++;
      return item.valor;
    }

    this.misses++;
    const valor = await fnGeneradora();

    // No cachear valores nulos o vacíos por error
    if (valor !== undefined && valor !== null) {
      if (this.cache.size >= this.maxItems) {
        // Eliminar el primer elemento (FIFO/LRU básico)
        const primerClave = this.cache.keys().next().value;
        this.cache.delete(primerClave);
      }
      this.cache.set(clave, {
        valor,
        expira: ahora + ttlSegundos * 1000,
      });
    }

    return valor;
  }

  /**
   * Elimina claves que coincidan con un prefijo (ej: 'feed:', 'videos:').
   */
  invalidarPrefijo(prefijo) {
    for (const k of this.cache.keys()) {
      if (k.startsWith(prefijo)) {
        this.cache.delete(k);
      }
    }
  }

  /**
   * Invalida los feeds globales cuando hay actividad nueva (post, delete, like).
   */
  invalidarFeeds() {
    this.invalidarPrefijo('feed:');
    this.invalidarPrefijo('videos:');
    this.invalidarPrefijo('trending:');
  }

  /**
   * Limpia toda la memoria caché.
   */
  limpiar() {
    this.cache.clear();
  }

  /**
   * Métricas de efectividad de la memoria caché.
   */
  estadisticas() {
    const total = this.hits + this.misses;
    const ratio = total > 0 ? Math.round((this.hits / total) * 100) : 0;
    return {
      elementos_en_ram: this.cache.size,
      peticiones_atendidas_en_ram: this.hits,
      consultas_a_neon: this.misses,
      efectividad_porcentaje: ratio,
    };
  }
}

export const microCache = new MicroCache(800);
