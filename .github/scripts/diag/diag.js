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
      if (window.__DIAG.refresh) localStorage.setItem('moon_refresh_token', window.__DIAG.refresh);
      if (window.__DIAG.user) localStorage.setItem('moon_user', JSON.stringify(window.__DIAG.user));
      info.sesionPuesta = 'si';
    } else {
      info.sesionPuesta = 'no (sin datos)';
    }
    if (location.hash !== '#/feed') location.hash = '#/feed';

    await hasta(() => document.querySelector('.post'));
    // La barra de abajo de la app (Inicio, Buscar…): el menu no debe montarsele.
    const barra = document.querySelector('.bottom-nav');
    info.barraAbajo = barra ? (() => { const rb = barra.getBoundingClientRect();
      return { arriba: Math.round(rb.top), alto: Math.round(rb.height), visible: rb.height > 0 }; })() : null;
    // El caso que falla es el de abajo del todo: se baja la página y se usa la
    // última publicación, cuya barra de acciones queda contra el borde.
    window.scrollTo(0, document.body.scrollHeight);
    await esperar(900);
    const todos = document.querySelectorAll('.post');
    const post = todos[todos.length - 1] || document.querySelector('.post');
    info.ruta = location.hash;
    info.hayFormularioDeEntrada = !!document.querySelector('input[type="password"]');
    info.raiz = (() => { const r = document.getElementById('root'); return r ? r.children.length + ' hijos: ' + Array.from(r.children).slice(0, 3).map(nombre).join(', ') : 'sin #root'; })();
    info.publicacionesEnPantalla = document.querySelectorAll('.post').length;
    if (!post) {
      info.error = 'no apareció ninguna publicación (¿sin sesión?)';
      info.pantallaMostrada = nombre(document.querySelector('.marco, .gate, main') || document.body.firstElementChild);
    } else {
      const boton = post.querySelector('button[aria-label="Más opciones de la publicación"]');
      info.hayBotonTresPuntos = !!boton;
      if (boton) boton.click();
      // El menú de opciones se dibuja fuera del post: se busca en toda la página.
      const item = await hasta(() => Array.from(document.querySelectorAll('.menu button')).find((b) => /Elegir reacci/.test(b.textContent)));
      const cajaOpciones = document.querySelector('.menu.opciones-flotantes');
      if (cajaOpciones) {
        const ro = cajaOpciones.getBoundingClientRect();
        info.menuOpcionesTapaLaBarra = (info.barraAbajo && info.barraAbajo.visible)
          ? ro.bottom > info.barraAbajo.arriba + 0.5 : null;
        info.menuOpciones = {
          rect: { x: Math.round(ro.x), y: Math.round(ro.y), ancho: Math.round(ro.width), alto: Math.round(ro.height), abajo: Math.round(ro.bottom) },
          dentroDePantalla: ro.top >= 0 && ro.left >= 0 && ro.bottom <= innerHeight + 1 && ro.right <= innerWidth + 1,
          recibeElToque: (() => {
            const el = document.elementFromPoint(Math.round(ro.left + ro.width / 2), Math.round(ro.top + 24));
            return el === cajaOpciones || !!(cajaOpciones.contains(el));
          })(),
          position: getComputedStyle(cajaOpciones).position,
          botones: cajaOpciones.querySelectorAll('button').length,
        };
      }
      info.hayOpcionElegirReaccion = !!item;
      if (item) {
        item.click();
        await esperar(1500);
        const menu = document.querySelector('.reacciones-menu');
        const overlay = document.querySelector('.hoja-fondo');
        info.hayOverlay = !!overlay;
        if (menu && info.barraAbajo && info.barraAbajo.visible) {
          info.menuEmojisTapaLaBarra = menu.getBoundingClientRect().bottom > info.barraAbajo.arriba + 0.5;
        }
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
          info.elMenuRecibeElToque = document.elementFromPoint(cx, cy) === menu
            || (document.elementFromPoint(cx, cy) && menu.contains(document.elementFromPoint(cx, cy)));
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
  pre.style.cssText = 'position:fixed;left:-99999px;top:0;z-index:-1';
  pre.textContent = 'DIAG-INICIO' + JSON.stringify(info, null, 1) + 'DIAG-FIN';
  document.body.appendChild(pre);
})();
