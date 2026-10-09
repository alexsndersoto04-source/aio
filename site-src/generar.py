#!/usr/bin/env python3
"""Genera site/index.html y site/biblioteca.html a partir de plantillas y de docs/STDLIB.md.

Uso: python3 site-src/generar.py   (desde la raíz del repositorio)
Los ejemplos de código están en site-src/ejemplos/*.titan y se ejecutaron con `titan run`;
la salida mostrada junto a cada uno es la real (site-src/ejemplos/*.out).
"""
import html, os, re

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.path.dirname(AQUI)
SALIDA = os.path.join(RAIZ, "site")
REPO = "https://github.com/alexsndersoto04-source/aio"


def leer(p):
    with open(p, encoding="utf-8") as f:
        return f.read()


def bloque(titulo, texto, salida=None):
    s = '<div class="code"><div class="cap">%s</div><pre><code>%s</code></pre>' % (
        html.escape(titulo), html.escape(texto.strip("\n")))
    if salida:
        s += '<pre class="out"><code>%s</code></pre>' % html.escape(salida.strip("\n"))
    return s + "</div>"


def ejemplo(nombre, titulo):
    return bloque(titulo, leer(os.path.join(AQUI, "ejemplos", nombre + ".titan")),
                  leer(os.path.join(AQUI, "ejemplos", nombre + ".out")))


def doc(ruta, texto):
    return '<a href="%s/blob/main/%s">%s</a>' % (REPO, ruta, texto)


def render(plantilla):
    # {{EJ:nombre|título}}  {{COD:título|texto}}  {{DOC:ruta|texto}}  {{REPO}}
    plantilla = re.sub(r"\{\{EJ:([a-z0-9_]+)\|([^}]*)\}\}", lambda m: ejemplo(m.group(1), m.group(2)), plantilla)
    plantilla = re.sub(r"\{\{COD:([^|}]*)\|([^}]*)\}\}", lambda m: bloque(m.group(1), m.group(2).replace("\\n", "\n")), plantilla)
    plantilla = re.sub(r"\{\{DOC:([^|}]*)\|([^}]*)\}\}", lambda m: doc(m.group(1), m.group(2)), plantilla)
    return plantilla.replace("{{REPO}}", REPO)


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


def pagina(nombre, titulo, descripcion, cuerpo):
    cab = leer(os.path.join(AQUI, "plantillas", "cabecera.html"))
    pie = leer(os.path.join(AQUI, "plantillas", "pie.html"))
    cab = cab.replace("{{TITULO}}", html.escape(titulo)).replace("{{DESC}}", html.escape(descripcion))
    with open(os.path.join(SALIDA, nombre), "w", encoding="utf-8") as f:
        f.write(render(cab + cuerpo + pie))


pagina("index.html", "Titan — lenguaje de programación",
       "Titan es un lenguaje de programación compilado y con tipos comprobados. Genera ejecutables nativos para "
       "Linux x86-64 y ARM64 (Termux) y WebAssembly, e incluye una biblioteca estándar de 812 funciones.",
       leer(os.path.join(AQUI, "plantillas", "inicio.html")))
pagina("biblioteca.html", "Biblioteca estándar — Titan",
       "Las 812 funciones de la biblioteca estándar de Titan, agrupadas en 73 espacios de nombres.",
       leer(os.path.join(AQUI, "plantillas", "biblioteca.html")).replace("{{FILAS}}", filas_biblioteca()))
print("ok")
