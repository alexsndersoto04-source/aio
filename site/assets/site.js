const translations = {
  es: {
    title: "Titan — lenguaje de programación",
    description: "Titan es un lenguaje compilado y tipado estáticamente, con bytecode portable, una VM segura y backends LLVM y WebAssembly. Descubre el compilador Zett y la documentación oficial.",
    skip: "Saltar al contenido",
    "brand.home": "Titan — inicio",
    "nav.aria": "Navegación principal",
    "nav.language": "Lenguaje",
    "nav.runtime": "Runtime",
    "nav.library": "Biblioteca",
    "nav.docs": "Documentación",
    "nav.github": "GitHub",
    "menu.open": "Abrir menú",
    "menu.close": "Cerrar menú",
    "hero.eyebrow": "LENGUAJE · RUNTIME · HERRAMIENTAS",
    "hero.title.one": "Programa con",
    "hero.title.two": "claridad y control.",
    "hero.lead": "Titan es un lenguaje compilado y tipado estáticamente, con bytecode portable y una máquina virtual segura. Zett es la distribución del compilador: el mismo ecosistema, desde el escritorio hasta Android.",
    "hero.download": "Descargar Zett",
    "hero.read": "Leer la especificación",
    "hero.release": "Release actual",
    "code.expected": "Salida esperada",
    "code.example": "Ejemplo de código Titan",
    "proof.aria": "Características principales",
    "proof.static": "Tipado estático",
    "proof.bytecode": "Bytecode validado",
    "proof.wasm": "Backend WebAssembly",
    "proof.termux": "Linux · macOS · Windows · Termux",
    "language.kicker": "EL LENGUAJE",
    "language.title": "Una base clara para programas reales.",
    "language.intro": "Titan combina una sintaxis expresiva con comprobaciones explícitas y un formato portable, para que los errores se encuentren antes de ejecutar.",
    "language.card1.title": "Tipos que ayudan",
    "language.card1.body": "Funciones, closures, structs, enums, métodos, traits, aliases y módulos con imports. Option y Result hacen visibles los casos que pueden fallar.",
    "language.card2.title": "Bytecode comprobado",
    "language.card2.body": "Los artefactos .tbc llevan versión y checksum; se validan antes de ejecutarse, incluidos saltos, aridad, locales y llamadas nativas.",
    "language.card3.title": "Un lenguaje, más destinos",
    "language.card3.body": "El compilador genera bytecode para la VM y cuenta con backends LLVM y WebAssembly para destinos adicionales.",
    "runtime.kicker": "RUNTIME Y SEGURIDAD",
    "runtime.title": "Control explícito, desde el bytecode hasta los efectos.",
    "runtime.intro": "La VM valida el programa antes de ejecutarlo y puede restringir operaciones del sistema mediante capacidades.",
    "runtime.link": "Conoce la arquitectura",
    "runtime.item1.title": "Validación antes de ejecutar",
    "runtime.item1.body": "Formato, checksum, tamaños, funciones, instrucciones, saltos y llamadas se comprueban antes de aceptar un .tbc.",
    "runtime.item2.title": "Sandbox por capacidades",
    "runtime.item2.body": "--sandbox deniega filesystem, procesos, red y environment, sin desactivar funciones puras.",
    "runtime.item3.title": "Concurrencia con límites claros",
    "runtime.item3.body": "Tareas sobre threads del host, canales acotados, cancelación cooperativa y cuotas de memoria por tarea.",
    "library.kicker": "BIBLIOTECA ESTÁNDAR",
    "library.title": "Herramientas para trabajar en contexto.",
    "library.intro": "Una API agrupada por capacidades: datos, seguridad, red, sistema, multimedia e interfaces.",
    "library.registry1": "firmas nativas registradas",
    "library.registry2": "namespaces std::",
    "library.registryLink": "Ver referencia",
    "tools.kicker": "EL ECOSISTEMA",
    "tools.title": "Del primer archivo al flujo de trabajo completo.",
    "tools.intro": "El repositorio incluye herramientas para editar, compilar, depurar y distribuir proyectos Titan.",
    "tools.cli": "Crear proyectos, comprobar tipos, ejecutar, compilar y probar.",
    "tools.lsp": "Diagnósticos, navegación, referencias y ayuda de firma.",
    "tools.dap": "Adaptador para clientes compatibles con Debug Adapter Protocol.",
    "tools.llvm": "Backend escrito en Titan: genera IR para Clang.",
    "tools.packages": "Paquetes .tpkg con verificación de integridad y firma Ed25519.",
    "wasm.kicker": "TITAN EN LA WEB",
    "wasm.title": "Un backend WebAssembly de verdad.",
    "wasm.body": "El backend emite módulos WASM y source maps. El host de navegador puede conectar DOM, eventos, fetch, WebSocket, Canvas 2D, animación y WebGL2 mediante JavaScript.",
    "wasm.link": "Leer sobre WebAssembly",
    "wasm.source": "código fuente",
    "wasm.module": "módulo validable",
    "wasm.host": "navegador / runtime",
    "install.kicker": "EMPIEZA CON TITAN",
    "install.title": "Instala Zett y ejecuta tu primer programa.",
    "install.body": "Elige el paquete de tu plataforma en los releases oficiales. También puedes instalar Zett desde Termux.",
    "install.all": "Todos los releases",
    "install.quickstart": "Inicio rápido · Linux x86-64",
    "install.copy": "Copiar comandos",
    "install.alsa": "El binario Linux necesita la biblioteca ALSA real (libasound.so.2). Instálala desde los paquetes de tu distribución si falta.",
    "platforms.aria": "Descargas por plataforma",
    "copy.done": "Copiado",
    "copy.unavailable": "Copia manualmente los comandos",
    "termux.title": "Instalación en Termux",
    "termux.body": "Añade el repositorio oficial y usa el gestor de paquetes. Para las integraciones Android, instala también termux-api y la app Termux:API.",
    "termux.guide": "Guía de Termux →",
    "termux.trust": "El origen APT usa trusted=yes, por lo que APT no verifica una firma del repositorio. Revisa la guía y el canal de distribución antes de instalar.",
    "selfhost.status": "EN DESARROLLO",
    "selfhost.kicker": "TRANSPARENCIA DEL PROYECTO",
    "selfhost.title": "El self-hosting sigue avanzando.",
    "selfhost.body": "El compilador y la VM de referencia actuales están implementados en Rust. El bootstrap nativo de Titan pasa la recompilación en tres generaciones; portar la biblioteca estándar y el runtime sigue en curso. El inventario estático registra fuente Titan para 518 de 816 firmas nativas, pero ese recuento no equivale a paridad de comportamiento.",
    "selfhost.link": "Ver estado, límites y pruebas",
    "docs.kicker": "DOCUMENTACIÓN",
    "docs.title": "Aprende el lenguaje desde sus fuentes.",
    "docs.intro": "Especificación, guías y ejemplos mantenidos junto al código del compilador.",
    "docs.spec": "Especificación del lenguaje",
    "docs.specSub": "Tipos, expresiones, módulos y semántica.",
    "docs.syntax": "Referencia de sintaxis",
    "docs.syntaxSub": "Guía práctica de construcción de programas.",
    "docs.stdlib": "Biblioteca estándar",
    "docs.stdlibSub": "Namespaces, capacidades y ejemplos.",
    "docs.projects": "Proyectos y paquetes",
    "docs.projectsSub": "Estructura, imports, pruebas y dependencias.",
    "docs.wasmSub": "Backend, memoria y host imports.",
    "docs.architecture": "Arquitectura",
    "docs.architectureSub": "Componentes del compilador y runtime.",
    "closing.kicker": "LISTO PARA EMPEZAR",
    "closing.title": "Escribe tu primer programa Titan.",
    "closing.button": "Descargar Zett",
    "footer.tagline": "Un lenguaje. Un compilador. Un ecosistema en construcción.",
    "footer.license": "Licencia MIT",
    "footer.releases": "Releases",
    "footer.note": "Hecho con claridad y software abierto."
  },
  en: {
    title: "Titan — programming language",
    description: "Titan is a statically typed, compiled language with portable bytecode, a secure VM, and LLVM and WebAssembly backends. Explore the Zett compiler distribution and official documentation.",
    skip: "Skip to content",
    "brand.home": "Titan — home",
    "nav.aria": "Main navigation",
    "nav.language": "Language",
    "nav.runtime": "Runtime",
    "nav.library": "Library",
    "nav.docs": "Documentation",
    "nav.github": "GitHub",
    "menu.open": "Open menu",
    "menu.close": "Close menu",
    "hero.eyebrow": "LANGUAGE · RUNTIME · TOOLING",
    "hero.title.one": "Program with",
    "hero.title.two": "clarity and control.",
    "hero.lead": "Titan is a statically typed, compiled language with portable bytecode and a secure virtual machine. Zett is the compiler distribution: one ecosystem, from desktop to Android.",
    "hero.download": "Download Zett",
    "hero.read": "Read the specification",
    "hero.release": "Latest release",
    "code.expected": "Expected output",
    "code.example": "Titan code example",
    "proof.aria": "Key features",
    "proof.static": "Static typing",
    "proof.bytecode": "Validated bytecode",
    "proof.wasm": "WebAssembly backend",
    "proof.termux": "Linux · macOS · Windows · Termux",
    "language.kicker": "THE LANGUAGE",
    "language.title": "A clear foundation for real programs.",
    "language.intro": "Titan combines expressive syntax with explicit checks and a portable format, so errors are found before execution.",
    "language.card1.title": "Types that help",
    "language.card1.body": "Functions, closures, structs, enums, methods, traits, aliases, and imported modules. Option and Result make failure cases explicit.",
    "language.card2.title": "Verified bytecode",
    "language.card2.body": "The .tbc artifacts include a version and checksum; jumps, arity, locals, and native calls are validated before execution.",
    "language.card3.title": "One language, more targets",
    "language.card3.body": "The compiler emits VM bytecode and includes LLVM and WebAssembly backends for additional targets.",
    "runtime.kicker": "RUNTIME & SECURITY",
    "runtime.title": "Explicit control, from bytecode to effects.",
    "runtime.intro": "The VM validates a program before execution and can restrict system operations through capabilities.",
    "runtime.link": "Explore the architecture",
    "runtime.item1.title": "Validation before execution",
    "runtime.item1.body": "Format, checksum, sizes, functions, instructions, jumps, and calls are checked before a .tbc is accepted.",
    "runtime.item2.title": "Capability-based sandbox",
    "runtime.item2.body": "--sandbox denies filesystem, processes, network, and environment access without disabling pure functions.",
    "runtime.item3.title": "Concurrency with clear limits",
    "runtime.item3.body": "Tasks use host threads, bounded channels, cooperative cancellation, and per-task memory quotas.",
    "library.kicker": "STANDARD LIBRARY",
    "library.title": "Tools that work across domains.",
    "library.intro": "An API organized by capability: data, security, networking, system, media, and interfaces.",
    "library.registry1": "registered native signatures",
    "library.registry2": "std:: namespaces",
    "library.registryLink": "View reference",
    "tools.kicker": "THE ECOSYSTEM",
    "tools.title": "From the first file to a complete workflow.",
    "tools.intro": "The repository includes tools to edit, compile, debug, and distribute Titan projects.",
    "tools.cli": "Create projects, check types, run, build, and test.",
    "tools.lsp": "Diagnostics, navigation, references, and signature help.",
    "tools.dap": "An adapter for clients that support the Debug Adapter Protocol.",
    "tools.llvm": "A Titan-written backend that emits IR for Clang.",
    "tools.packages": ".tpkg packages with integrity checks and Ed25519 signatures.",
    "wasm.kicker": "TITAN ON THE WEB",
    "wasm.title": "A real WebAssembly backend.",
    "wasm.body": "The backend emits WASM modules and source maps. A browser host can connect the DOM, events, fetch, WebSocket, Canvas 2D, animation, and WebGL2 through JavaScript.",
    "wasm.link": "Read about WebAssembly",
    "wasm.source": "source code",
    "wasm.module": "validated module",
    "wasm.host": "browser / runtime",
    "install.kicker": "GET STARTED WITH TITAN",
    "install.title": "Install Zett and run your first program.",
    "install.body": "Choose the package for your platform from the official releases. Zett is also available for Termux.",
    "install.all": "All releases",
    "install.quickstart": "Quick start · Linux x86-64",
    "install.copy": "Copy commands",
    "install.alsa": "The Linux binary requires the real ALSA library (libasound.so.2). Install it from your distribution's package manager if it is missing.",
    "platforms.aria": "Platform downloads",
    "copy.done": "Copied",
    "copy.unavailable": "Copy the commands manually",
    "termux.title": "Install on Termux",
    "termux.body": "Add the official repository and use its package manager. For Android integrations, also install termux-api and the Termux:API app.",
    "termux.guide": "Termux guide →",
    "termux.trust": "The APT source uses trusted=yes, so APT does not verify a repository signature. Review the guide and distribution channel before installing.",
    "selfhost.status": "IN PROGRESS",
    "selfhost.kicker": "PROJECT STATUS",
    "selfhost.title": "Self-hosting is moving forward.",
    "selfhost.body": "The current reference compiler and VM are implemented in Rust. Titan's native bootstrap passes three-generation recompilation; porting the standard library and runtime is still in progress. The static inventory lists Titan source for 518 of 816 native signatures, but this count does not mean behavioral parity.",
    "selfhost.link": "View status, limits, and tests",
    "docs.kicker": "DOCUMENTATION",
    "docs.title": "Learn the language from its sources.",
    "docs.intro": "Specifications, guides, and examples maintained alongside the compiler.",
    "docs.spec": "Language specification",
    "docs.specSub": "Types, expressions, modules, and semantics.",
    "docs.syntax": "Syntax reference",
    "docs.syntaxSub": "A practical guide to writing Titan programs.",
    "docs.stdlib": "Standard library",
    "docs.stdlibSub": "Namespaces, capabilities, and examples.",
    "docs.projects": "Projects and packages",
    "docs.projectsSub": "Structure, imports, tests, and dependencies.",
    "docs.wasmSub": "Backend, memory, and host imports.",
    "docs.architecture": "Architecture",
    "docs.architectureSub": "Compiler and runtime components.",
    "closing.kicker": "READY TO START",
    "closing.title": "Write your first Titan program.",
    "closing.button": "Download Zett",
    "footer.tagline": "One language. One compiler. An ecosystem in progress.",
    "footer.license": "MIT License",
    "footer.releases": "Releases",
    "footer.note": "Built with clarity and open software."
  }
};

let currentLanguage = "es";
const languageButton = document.getElementById("language-toggle");
const menuButton = document.getElementById("menu-toggle");
const nav = document.getElementById("site-nav");
const copyButton = document.getElementById("copy-command");
const copyStatus = document.getElementById("copy-status");

function applyLanguage(language) {
  currentLanguage = language;
  document.documentElement.lang = language;
  document.title = translations[language].title;
  const description = document.querySelector('meta[name="description"]');
  if (description) description.content = translations[language].description;

  document.querySelectorAll("[data-i18n]").forEach((element) => {
    const value = translations[language][element.dataset.i18n];
    if (value !== undefined) element.textContent = value;
  });
  document.querySelectorAll("[data-i18n-aria]").forEach((element) => {
    const value = translations[language][element.dataset.i18nAria];
    if (value !== undefined) element.setAttribute("aria-label", value);
  });

  languageButton.textContent = language === "es" ? "EN" : "ES";
  languageButton.setAttribute("aria-label", language === "es" ? "Switch to English" : "Cambiar a español");
  if (menuButton) {
    const isOpen = menuButton.getAttribute("aria-expanded") === "true";
    menuButton.setAttribute("aria-label", translations[language][isOpen ? "menu.close" : "menu.open"]);
  }
  if (copyStatus) copyStatus.textContent = "";
}

languageButton.addEventListener("click", () => {
  applyLanguage(currentLanguage === "es" ? "en" : "es");
});

menuButton.addEventListener("click", () => {
  const isOpen = menuButton.getAttribute("aria-expanded") === "true";
  menuButton.setAttribute("aria-expanded", String(!isOpen));
  menuButton.setAttribute("aria-label", translations[currentLanguage][isOpen ? "menu.open" : "menu.close"]);
  nav.classList.toggle("is-open", !isOpen);
});

nav.querySelectorAll("a").forEach((link) => {
  link.addEventListener("click", () => {
    nav.classList.remove("is-open");
    menuButton.setAttribute("aria-expanded", "false");
    menuButton.setAttribute("aria-label", translations[currentLanguage]["menu.open"]);
  });
});

document.addEventListener("keydown", (event) => {
  if (event.key === "Escape" && nav.classList.contains("is-open")) {
    nav.classList.remove("is-open");
    menuButton.setAttribute("aria-expanded", "false");
    menuButton.setAttribute("aria-label", translations[currentLanguage]["menu.open"]);
    menuButton.focus();
  }
});

copyButton.addEventListener("click", async () => {
  const code = document.getElementById("quickstart-code").textContent;
  try {
    if (!navigator.clipboard || !window.isSecureContext) throw new Error("Clipboard API unavailable");
    await navigator.clipboard.writeText(code);
    copyStatus.textContent = translations[currentLanguage]["copy.done"];
  } catch {
    copyStatus.textContent = translations[currentLanguage]["copy.unavailable"];
  }
});

applyLanguage("es");
