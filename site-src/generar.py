#!/usr/bin/env python3
"""Genera el sitio estático de Titan (site/) a partir de plantillas, de docs/*.md y de selfhost/ESTADO.md.

Uso (desde la raíz del repositorio):
    python3 site-src/generar.py [variante] [carpeta_de_salida]
        variante: claro (por defecto); carpeta por defecto: site/
Requiere el paquete `markdown` (pip install markdown).

Los ejemplos de código están en site-src/ejemplos/*.titan y se ejecutaron con `titan run`;
la salida mostrada junto a cada uno es la real (site-src/ejemplos/*.out).
"""
import html, os, re, shutil, sys
import markdown

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.path.dirname(AQUI)
REPO = "https://github.com/alexsndersoto04-source/aio"
VARIANTE = sys.argv[1] if len(sys.argv) > 1 else "claro"
SALIDA = sys.argv[2] if len(sys.argv) > 2 else os.path.join(RAIZ, "site")
os.makedirs(os.path.join(SALIDA, "docs"), exist_ok=True)

# ---------------------------------------------------------------- documentación a publicar
# (archivo en el repositorio, nombre de la página, título del menú, descripción)
GRUPOS = [
    ("Empezar", [
        ("docs/GUIA_RAPIDA.md", "guia-rapida", "Guía rápida", "Recorrido por el lenguaje en 20 minutos, con programas que se ejecutan."),
        ("docs/GUIA_TERMUX.md", "termux", "Titan en Termux", "Instalar y usar Titan en Android (ARM64)."),
        ("docs/PROJECTS.md", "proyectos", "Proyectos, módulos y tests", "Estructura de un proyecto, Titan.toml, imports y pruebas."),
        ("docs/ARCHITECTURE.md", "arquitectura", "Arquitectura", "Etapas del compilador y generadores de código."),
    ]),
    ("Lenguaje", [
        ("docs/TITAN_SYNTAX.md", "sintaxis", "Referencia de sintaxis", "Todas las construcciones del lenguaje con ejemplos."),
        ("docs/SPEC.md", "especificacion", "Especificación", "Definición formal del lenguaje y de sus reglas de tipos (en inglés)."),
        ("docs/CONCURRENCY.md", "concurrencia", "Tareas y canales", "spawn, join, canales, select y tiempos límite."),
    ]),
    ("Biblioteca estándar", [
        ("docs/STDLIB.md", "stdlib", "Visión general", "Los 73 espacios de nombres y sus 812 funciones."),
        ("docs/EXTRAS.md", "extras", "Regex, uuid, hash y más", "Módulos de uso general."),
        ("docs/NETWORKING.md", "red", "Red y HTTP", "Sockets, servidor HTTP, enrutador y respuestas."),
        ("docs/HTTP_CLIENT.md", "cliente-http", "Cliente HTTP/HTTPS", "std::http::request."),
        ("docs/TLS.md", "tls", "TLS", "Conexiones seguras y credenciales de servidor."),
        ("docs/WEBSOCKET.md", "websocket", "WebSocket", "Cliente, servidor y decodificador de mensajes."),
        ("docs/DATABASE_API.md", "base-de-datos", "API común de bases de datos", "std::db y pools."),
        ("docs/SQLITE.md", "sqlite", "SQLite", "Motor SQL propio, migraciones y límites."),
        ("docs/POSTGRESQL.md", "postgresql", "PostgreSQL", "Conexión, consultas y TLS."),
        ("docs/MYSQL.md", "mysql", "MySQL", "Conexión, consultas y transacciones."),
        ("docs/MULTIPART.md", "multipart", "Subida de archivos", "Formularios multipart."),
        ("docs/SERVER_LIFECYCLE.md", "servidor", "Ciclo de vida del servidor", "Arranque, parada y presión de carga."),
        ("docs/METRICS.md", "metricas", "Métricas", "Contadores, gauges e histogramas."),
        ("docs/AUDIO.md", "audio", "Audio", "WAV, etiquetas, biblioteca y síntesis."),
        ("docs/AUDIO_ENGINE.md", "motor-de-audio", "Motor de audio", "Decodificación y reproducción."),
        ("docs/WASM.md", "wasm", "WebAssembly", "Backend WASM, memoria y host JavaScript."),
    ]),
    ("Herramientas", [
        ("docs/ZETT.md", "zett", "Zett, el gestor de paquetes", "Añadir, instalar, firmar y publicar paquetes."),
        ("docs/LSP.md", "lsp", "Servidor de lenguaje (LSP)", "Diagnósticos, símbolos y renombrado en editores."),
        ("docs/DAP.md", "dap", "Adaptador de depuración (DAP)", "Depuración desde editores."),
        ("docs/DEBUGGER.md", "depurador", "Depurador", "Información de depuración y depurador interactivo."),
        ("docs/PACKAGE_REGISTRY.md", "paquetes", "Paquetes", "Registro remoto, firmas y dependencias."),
    ]),
    ("Estado del proyecto", [
        ("selfhost/ESTADO.md", "estado", "Estado verificado", "Qué se probó, cómo, y cada limitación conocida."),
        ("CHANGELOG.md", "cambios", "Historial de cambios", "Cambios por versión."),
        ("docs/VALIDATION.md", "validacion", "Registro de validación (histórico)", "Pruebas de la etapa en Rust."),
    ]),
]
PAGINAS = {}  # ruta del repo -> (nombre, titulo)
for _g, items in GRUPOS:
    for ruta, nombre, titulo, _d in items:
        PAGINAS[ruta] = (nombre, titulo)


def leer(p):
    with open(p, encoding="utf-8") as f:
        return f.read()


# ---------------------------------------------------------------- resaltado de código Titan
KW = ("fn let mut if else match while for in return enum struct impl trait import const type loop break continue "
      "true false nil spawn pub use as").split()
TOK = re.compile(r'(//[^\n]*)|("(?:\\.|[^"\\])*")|(\b\d[\d_]*(?:\.\d+)?\b)|(\b(?:%s)\b)|(\b[A-Z][A-Za-z0-9_]*(?=::|\())' % "|".join(KW))


def resaltar_titan(texto):
    out, pos = [], 0
    for m in TOK.finditer(texto):
        out.append(html.escape(texto[pos:m.start()]))
        clase = "c" if m.group(1) else "s" if m.group(2) else "n" if m.group(3) else "k" if m.group(4) else "t"
        out.append('<span class="%s">%s</span>' % (clase, html.escape(m.group(0))))
        pos = m.end()
    out.append(html.escape(texto[pos:]))
    return "".join(out)


def bloque(titulo, texto, salida=None, lang=None):
    texto = texto.strip("\n")
    cuerpo = resaltar_titan(texto) if lang == "titan" else html.escape(texto)
    s = '<div class="code"><div class="cap">%s</div><pre><code>%s</code></pre>' % (html.escape(titulo), cuerpo)
    if salida:
        s += '<div class="salida-et">Salida</div><pre class="out"><code>%s</code></pre>' % html.escape(salida.strip("\n"))
    return s + "</div>"


def ejemplo(nombre, titulo):
    return bloque(titulo, leer(os.path.join(AQUI, "ejemplos", nombre + ".titan")),
                  leer(os.path.join(AQUI, "ejemplos", nombre + ".out")), "titan")


def doc(ruta, texto):
    return '<a href="%s/blob/main/%s">%s</a>' % (REPO, ruta, texto)


def render(plantilla, base):
    plantilla = re.sub(r"\{\{EJ:([a-z0-9_]+)\|([^}]*)\}\}", lambda m: ejemplo(m.group(1), m.group(2)), plantilla)
    plantilla = re.sub(r"\{\{COD:([^|}]*)\|([^}]*)\}\}", lambda m: bloque(m.group(1), m.group(2).replace("\\n", "\n")), plantilla)
    plantilla = re.sub(r"\{\{DOC:([^|}]*)\|([^}]*)\}\}", lambda m: doc(m.group(1), m.group(2)), plantilla)
    return plantilla.replace("{{REPO}}", REPO).replace("{{BASE}}", base)


def cabecera(titulo, descripcion, base, activo=""):
    c = leer(os.path.join(AQUI, "plantillas", "cabecera.html"))
    c = c.replace("{{TITULO}}", html.escape(titulo)).replace("{{DESC}}", html.escape(descripcion))
    return c.replace('data-a="%s"' % activo, 'aria-current="page"')


def pie():
    return leer(os.path.join(AQUI, "plantillas", "pie.html"))


def escribir(ruta_rel, contenido):
    with open(os.path.join(SALIDA, ruta_rel), "w", encoding="utf-8") as f:
        f.write(contenido)


# ---------------------------------------------------------------- páginas principales
def pagina(nombre, titulo, descripcion, cuerpo, activo=""):
    escribir(nombre, render(cabecera(titulo, descripcion, "", activo) + cuerpo + pie(), ""))


def filas_biblioteca():
    out, total = [], 0
    for l in leer(os.path.join(RAIZ, "docs", "STDLIB.md")).splitlines():
        if not l.startswith("| `std::"):
            continue
        p = [x.strip() for x in l.strip().strip("|").split("|")]
        n = int(p[1])
        total += n
        out.append('<tr><td><code>%s</code></td><td class="num">%d</td><td>%s</td></tr>' % (
            html.escape(p[0].strip("`")), n, html.escape(p[2])))
    assert total == 812 and len(out) == 73, (total, len(out))
    return "\n".join(out)


pagina("index.html", "Titan — lenguaje de programación",
       "Titan es un lenguaje de programación compilado y con tipos comprobados. Genera ejecutables nativos para "
       "Linux x86-64 y ARM64 (Termux) y WebAssembly, e incluye una biblioteca estándar de 812 funciones.",
       leer(os.path.join(AQUI, "plantillas", "inicio.html")), "inicio")
pagina("biblioteca.html", "Biblioteca estándar — Titan",
       "Las 812 funciones de la biblioteca estándar de Titan, agrupadas en 73 espacios de nombres.",
       leer(os.path.join(AQUI, "plantillas", "biblioteca.html")).replace("{{FILAS}}", filas_biblioteca()), "biblioteca")

# ---------------------------------------------------------------- documentación
def menu_docs(actual):
    s = ['<nav class="side" aria-label="Documentación"><a class="side-home%s" href="%sdocumentacion.html">Índice</a>' % (
        " on" if actual == "" else "", "../" if actual else "")]
    for grupo, items in GRUPOS:
        s.append('<div class="side-g">%s</div><ul>' % html.escape(grupo))
        for _r, nombre, titulo, _d in items:
            href = ("%s.html" % nombre) if actual else ("docs/%s.html" % nombre)
            s.append('<li><a href="%s"%s>%s</a></li>' % (href, ' class="on"' if nombre == actual else "", html.escape(titulo)))
        s.append("</ul>")
    s.append("</nav>")
    return "\n".join(s)


def reescribir_enlaces(h, ruta_origen):
    base_dir = os.path.dirname(ruta_origen)

    def f(m):
        url = m.group(1)
        if re.match(r"^(https?:|mailto:|#)", url):
            return m.group(0)
        frag = ""
        if "#" in url:
            url, frag = url.split("#", 1)
            frag = "#" + frag
        destino = os.path.normpath(os.path.join(base_dir, url))
        if destino in PAGINAS:
            return 'href="%s.html%s"' % (PAGINAS[destino][0], frag)
        if os.path.isdir(os.path.join(RAIZ, destino)):
            return 'href="%s/tree/main/%s"' % (REPO, destino)
        return 'href="%s/blob/main/%s%s"' % (REPO, destino, frag)
    return re.sub(r'href="([^"]*)"', f, h)


def md_a_html(texto):
    md = markdown.Markdown(extensions=["tables", "fenced_code", "toc", "sane_lists"],
                           extension_configs={"toc": {"permalink": False}})
    h = md.convert(texto)

    def cod(m):
        lang, cuerpo = m.group(1), html.unescape(m.group(2))
        if lang == "titan":
            return '<pre><code class="language-titan">%s</code></pre>' % resaltar_titan(cuerpo)
        return m.group(0)
    h = re.sub(r'<pre><code class="language-([a-z]+)">(.*?)</code></pre>', cod, h, flags=re.S)
    h = h.replace("<table>", '<div class="tablewrap"><table>').replace("</table>", "</table></div>")
    return h


for grupo, items in GRUPOS:
    for ruta, nombre, titulo, descripcion in items:
        h = reescribir_enlaces(md_a_html(leer(os.path.join(RAIZ, ruta))), ruta)
        cuerpo = ('<div class="docs wrap"><aside>%s</aside><article class="md">'
                  '<p class="crumb"><a href="../documentacion.html">Documentación</a> / %s</p>%s'
                  '<p class="editar"><a href="%s/blob/main/%s">Ver este archivo en GitHub</a></p></article></div>'
                  % (menu_docs(nombre), html.escape(grupo), h, REPO, ruta))
        escribir("docs/%s.html" % nombre,
                 render(cabecera("%s — Documentación de Titan" % titulo, descripcion, "../", "docs") + cuerpo + pie(), "../"))

# índice de documentación
cuerpo = ['<div class="docs wrap"><aside>%s</aside><article class="md"><h1>Documentación</h1>' % menu_docs("")]
cuerpo.append('<p class="sub">Guías y referencias de Titan. Las páginas se generan desde la carpeta <code>docs/</code> del repositorio. '
              'Algunas se escribieron para el prototipo en Rust y lo indican al principio: la lista de lo que está probado y lo que no '
              'está en «Estado verificado».</p>')
for grupo, items in GRUPOS:
    cuerpo.append("<h2>%s</h2><div class=\"doclist\">" % html.escape(grupo))
    for _r, nombre, titulo, d in items:
        cuerpo.append('<a class="docitem" href="docs/%s.html"><b>%s</b><span>%s</span></a>' % (nombre, html.escape(titulo), html.escape(d)))
    cuerpo.append("</div>")
cuerpo.append("</article></div>")
escribir("documentacion.html", render(cabecera("Documentación — Titan", "Guías y referencias del lenguaje Titan.", "", "docs")
                                      + "\n".join(cuerpo) + pie(), ""))

# ---------------------------------------------------------------- estilos y recursos
css = leer(os.path.join(AQUI, "estilos", "base.css")) + "\n" + leer(os.path.join(AQUI, "estilos", VARIANTE + ".css"))
escribir("style.css", css)
for destino in ("favicon.svg", "zett-marca.svg"):
    shutil.copy(os.path.join(AQUI, "recursos", "zett-marca.svg"), os.path.join(SALIDA, destino))
open(os.path.join(SALIDA, ".nojekyll"), "w").close()
print("ok", VARIANTE, SALIDA)
