// Sonda del menú de reacciones: abre el menú «⋯», elige «Elegir reacción» y
// cuenta qué se ve de verdad en la pantalla (posiciones, capas y quién tapa a quién).
(async () => {
  const esperar = (ms) => new Promise((r) => setTimeout(r, ms));
  const hasta = (fn, ms = 25000) => new Promise((res) => {
    const t0 = Date.now();
    (function mirar() {
      let v = null;
      try { v = fn(); } catch (e) { v = null; }
      if (v) return res(v);
      if (Date.now() - t0 > ms) return res(null);
      setTimeout(mirar, 200);
    })();
  });
  const nombre = (el) => !el ? null : (typeof el.className === 'string' && el.className ? el.className.split(' ').slice(0, 2).join('.') : el.tagName);

  const info = { url: location.href, pantalla: innerWidth + 'x' + innerHeight, tema: document.documentElement.dataset.theme || '(claro)' };

  try {
    if (window.__DIAG && window.__DIAG.token) {
      localStorage.setItem('moon_access_token', window.__DIAG.token);
      if (window.__DIAG.user) localStorage.setItem('moon_user', JSON.stringify(window.__DIAG.user));
    }

    const post = await hasta(() => document.querySelector('.post'));
    info.publicacionesEnPantalla = document.querySelectorAll('.post').length;
    if (!post) {
      info.error = 'no apareció ninguna publicación (¿sin sesión?)';
      info.pantallaMostrada = nombre(document.querySelector('.marco, .gate, main') || document.body.firstElementChild);
    } else {
      const boton = post.querySelector('button[aria-label="Más opciones de la publicación"]');
      info.hayBotonTresPuntos = !!boton;
      if (boton) boton.click();
      const item = await hasta(() => Array.from(document.querySelectorAll('.post .menu button')).find((b) => /Elegir reacci/.test(b.textContent)));
      info.hayOpcionElegirReaccion = !!item;
      if (item) {
        item.click();
        await esperar(1500);
        const menu = document.querySelector('.reacciones-menu');
        const overlay = document.querySelector('.hoja-fondo');
        info.hayOverlay = !!overlay;
        info.hayMenu = !!menu;
        info.menusEnPagina = document.querySelectorAll('.reacciones-menu').length;
        info.botonesDeEmoji = menu ? menu.querySelectorAll('button').length : 0;
        if (overlay) {
          const ro = overlay.getBoundingClientRect();
          const so = getComputedStyle(overlay);
          info.overlayRect = { x: Math.round(ro.x), y: Math.round(ro.y), ancho: Math.round(ro.width), alto: Math.round(ro.height) };
          info.overlayEstilo = { position: so.position, zIndex: so.zIndex, backdrop: so.backdropFilter };
        }
        if (menu) {
          const r = menu.getBoundingClientRect();
          const s = getComputedStyle(menu);
          info.menuRect = { x: Math.round(r.x), y: Math.round(r.y), ancho: Math.round(r.width), alto: Math.round(r.height), derecha: Math.round(r.right), abajo: Math.round(r.bottom) };
          info.menuEstilo = { position: s.position, zIndex: s.zIndex, display: s.display, opacity: s.opacity, visibility: s.visibility, fondo: s.backgroundColor, bottom: s.bottom, overflow: s.overflow };
          info.menuDentroDePantalla = r.top >= 0 && r.left >= 0 && r.bottom <= innerHeight + 1 && r.right <= innerWidth + 1;
          info.primerEmoji = menu.querySelector('.moon-emoji') ? menu.querySelector('.moon-emoji').textContent : null;
          info.anchoDelEmoji = menu.querySelector('.moon-emoji') ? Math.round(menu.querySelector('.moon-emoji').getBoundingClientRect().width) : null;
          const cx = Math.round(r.left + r.width / 2);
          const cy = Math.round(r.top + r.height / 2);
          info.arribaEnElCentroDelMenu = nombre(document.elementFromPoint(cx, cy));
          info.arribaEnBotonEmoji = (() => {
            const b = menu.querySelector('button');
            if (!b) return null;
            const rb = b.getBoundingClientRect();
            return nombre(document.elementFromPoint(Math.round(rb.left + rb.width / 2), Math.round(rb.top + rb.height / 2)));
          })();
          info.cadena = [];
          for (let el = menu; el && el !== document.documentElement; el = el.parentElement) {
            const s = getComputedStyle(el);
            info.cadena.push({
              quien: nombre(el), position: s.position, zIndex: s.zIndex,
              overflow: s.overflow, transform: s.transform === 'none' ? '-' : s.transform,
              filter: s.filter === 'none' ? '-' : 'si', backdrop: s.backdropFilter === 'none' ? '-' : 'si',
              contain: s.contain, isolation: s.isolation, opacity: s.opacity,
            });
          }
          info.scrollDeLaPagina = Math.round(scrollY);
        }
        info.dentroDelPost = (() => {
          const el = document.querySelector('.reaccion-caja');
          if (!el) return null;
          const s = getComputedStyle(el);
          return { alto: Math.round(el.getBoundingClientRect().height), ancho: Math.round(el.getBoundingClientRect().width), position: s.position, zIndex: s.zIndex, contenidoRecortado: s.overflow };
        })();
      }
    }
  } catch (e) {
    info.excepcion = String(e && e.stack ? e.stack.split('\n').slice(0, 2).join(' | ') : e);
  }

  const pre = document.createElement('pre');
  pre.id = 'diag';
  pre.style.cssText = 'position:fixed;left:0;top:0;z-index:99999';
  pre.textContent = 'DIAG-INICIO' + JSON.stringify(info, null, 1) + 'DIAG-FIN';
  document.body.appendChild(pre);
})();
