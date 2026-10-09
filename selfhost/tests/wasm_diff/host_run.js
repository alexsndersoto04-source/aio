// Ejecuta un .wasm compilado de examples/browser/main.titan con el host.js REAL
// (examples/browser/host.js) sobre un DOM mínimo de Node (sin navegador) y
// escribe el registro de todo lo que el programa hizo en el DOM.
// Uso: node host_run.js <programa.wasm>
const fs = require("fs");
const path = require("path");
const vm = require("vm");
const browser = path.resolve(__dirname, "../../../examples/browser");
const wasmBytes = fs.readFileSync(process.argv[2]);
const log = [];
const listeners = [];
function element(sel) {
    const sandbox = globalThis.__sandbox;
    const state = sel.includes("scene") ? Object.create(sandbox.HTMLCanvasElement.prototype) : {};
    Object.assign(state, { textContent: "", innerHTML: "", width: 640, height: 240, value: "" });
    const classes = new Set();
    const attrs = {};
    const ctx = new Proxy({}, {
        get(_, p) { return (...a) => { log.push([sel, "ctx." + String(p), a]); return { width: 10 }; }; },
        set(_, p, v) { log.push([sel, "ctx." + String(p) + "=", v]); return true; },
    });
    return new Proxy(state, {
        get(t, p) {
            if (p === "classList") return { add: c => { classes.add(c); log.push([sel, "class+", c]); }, remove: c => { classes.delete(c); log.push([sel, "class-", c]); } };
            if (p === "setAttribute") return (k, v) => { attrs[k] = v; log.push([sel, "attr", k, v]); };
            if (p === "addEventListener") return (ev, cb) => { listeners.push([sel, ev, cb]); log.push([sel, "listen", ev]); };
            if (p === "removeEventListener") return () => {};
            if (p === "focus") return () => log.push([sel, "focus"]);
            if (p === "getContext") return kind => kind === "2d" ? ctx : null;
            if (p === "getBoundingClientRect") return () => ({ left: 0, top: 0, width: 640, height: 240 });
            return t[p];
        },
        set(t, p, v) { t[p] = v; if (p === "textContent" || p === "innerHTML") log.push([sel, String(p), v]); return true; },
    });
}
const nodes = new Map();
const document = {
    title: "",
    querySelector(sel) { if (!nodes.has(sel)) nodes.set(sel, element(sel)); return nodes.get(sel); },
};
let frame = null;
const sandbox = {
    document, console, TextDecoder, TextEncoder, WebAssembly, Map, Set, BigInt, Number, Math, JSON, Array, Uint8Array,
    Float32Array, Uint16Array, RangeError, TypeError, Error, Promise, URL, String, Object, Symbol, setTimeout, clearTimeout,
    AbortController, Headers, Request, Response, ReadableStream,
    requestAnimationFrame(cb) { frame = cb; return 1; },
    cancelAnimationFrame() { frame = null; },
    fetch: async (url) => {
        if (String(url).endsWith("program.wasm")) return { ok: true, status: 200, arrayBuffer: async () => wasmBytes };
        const body = fs.readFileSync(path.join(browser, String(url)));
        return { ok: true, status: 200, url: String(url), headers: { forEach: f => f("application/json", "content-type"), entries: () => [["content-type", "application/json"]] }, arrayBuffer: async () => body, text: async () => body.toString() };
    },
    WebSocket: class { constructor() { throw new Error("sin red en esta prueba"); } },
    HTMLCanvasElement: class {},
};
sandbox.globalThis = sandbox;
sandbox.window = sandbox;
const out = [];
sandbox.console = { log: (...a) => out.push(["console.log", ...a.map(String)]), error: (...a) => out.push(["console.error", ...a.map(e => (e && e.stack) || String(e))]) };
globalThis.__sandbox = sandbox;
vm.createContext(sandbox);
vm.runInContext(fs.readFileSync(path.join(browser, "host.js"), "utf8"), sandbox);
const tick = () => new Promise(r => setTimeout(r, 50));
(async () => {
    await tick(); await tick();
    for (const [sel, ev, cb] of listeners) {
        if (ev === "click" || ev === "input") {
            nodes.get(sel).value = "valor de prueba";
            cb({ type: ev, target: { id: sel.slice(1), value: "valor de prueba" }, clientX: 3, clientY: 4 });
            await tick();
        }
    }
    if (frame) { frame(16); frame = null; }
    await tick();
    console.log(JSON.stringify({ out, dom: log }, (k, v) => typeof v === "bigint" ? v.toString() : v, 1));
})();
