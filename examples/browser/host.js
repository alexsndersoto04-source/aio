const decoder = new TextDecoder("utf-8", { fatal: true });
const responseDecoder = new TextDecoder("utf-8");
const encoder = new TextEncoder();
let instance;
let currentEvent = null;
let currentFetch = null;
let nextListenerId = 1;
let nextFetchId = 1;
const listeners = new Map();
const requests = new Map();

function titanString(handle) {
    const bits = BigInt.asUintN(64, handle);
    const pointer = Number(bits & 0xffffffffn);
    const length = Number(bits >> 32n);
    const end = pointer + length;
    const memory = instance.exports.memory;
    if (pointer < 0 || length < 0 || end > memory.buffer.byteLength) {
        throw new RangeError("invalid TITAN string handle");
    }
    return decoder.decode(new Uint8Array(memory.buffer, pointer, length));
}

function wasmString(value) {
    const text = String(value);
    const bytes = encoder.encode(text);
    const scalarLength = Array.from(text).length;
    const allocate = instance.exports.__titan_alloc_string;
    if (typeof allocate !== "function") {
        throw new Error("module does not export __titan_alloc_string");
    }
    const handle = allocate(bytes.length, scalarLength);
    const bits = BigInt.asUintN(64, handle);
    const pointer = Number(bits & 0xffffffffn);
    const length = Number(bits >> 32n);
    if (length !== bytes.length || pointer + length > instance.exports.memory.buffer.byteLength) {
        throw new RangeError("TITAN allocator returned an invalid string handle");
    }
    new Uint8Array(instance.exports.memory.buffer, pointer, length).set(bytes);
    return handle;
}

function eventString(read) {
    return wasmString(currentEvent ? read(currentEvent) : "");
}

function fetchString(read) {
    return wasmString(currentFetch ? read(currentFetch) : "");
}

function safeInteger(raw, name, minimum = 0) {
    const value = typeof raw === "bigint" ? raw : BigInt(raw);
    if (value < BigInt(minimum) || value > BigInt(Number.MAX_SAFE_INTEGER)) {
        throw new RangeError(`${name} is outside the supported range`);
    }
    return Number(value);
}

function element(handle) {
    const selector = titanString(handle);
    const node = document.querySelector(selector);
    if (!node) throw new Error(`DOM selector did not match: ${selector}`);
    return node;
}

async function readBoundedBody(response, maximumBytes) {
    if (!response.body) {
        const bytes = new Uint8Array(await response.arrayBuffer());
        if (bytes.length > maximumBytes) throw new RangeError("fetch response exceeds maximumBytes");
        return responseDecoder.decode(bytes);
    }
    const reader = response.body.getReader();
    const chunks = [];
    let total = 0;
    while (true) {
        const { done, value } = await reader.read();
        if (done) break;
        total += value.length;
        if (total > maximumBytes) {
            await reader.cancel("response limit exceeded");
            throw new RangeError("fetch response exceeds maximumBytes");
        }
        chunks.push(value);
    }
    const bytes = new Uint8Array(total);
    let offset = 0;
    for (const chunk of chunks) {
        bytes.set(chunk, offset);
        offset += chunk.length;
    }
    return responseDecoder.decode(bytes);
}

function invokeFetchHandler(handlerName, context) {
    const previousFetch = currentFetch;
    currentFetch = context;
    try {
        const handler = instance.exports[handlerName];
        if (typeof handler !== "function") {
            throw new Error(`TITAN fetch handler is not exported: ${handlerName}`);
        }
        handler();
    } catch (error) {
        console.error(`TITAN fetch handler '${handlerName}' failed`, error);
    } finally {
        currentFetch = previousFetch;
    }
}

async function performFetch(id, url, maximumBytes, handlerName, record) {
    try {
        const response = await globalThis.fetch(url, { signal: record.controller.signal });
        const body = await readBoundedBody(response, maximumBytes);
        if (!record.cancelled) {
            invokeFetchHandler(handlerName, {
                ok: response.ok,
                status: response.status,
                body,
                url: response.url,
                error: "",
            });
        }
    } catch (error) {
        if (!record.cancelled) {
            invokeFetchHandler(handlerName, {
                ok: false,
                status: 0,
                body: "",
                url,
                error: record.timedOut ? "request timed out" : String(error?.message || error),
            });
        }
    } finally {
        if (record.timer !== null) clearTimeout(record.timer);
        if (requests.get(id) === record) requests.delete(id);
    }
}

const imports = {
    titan: {
        print(value) {
            console.log(titanString(value));
        },
        dom_query_exists(selector) {
            return document.querySelector(titanString(selector)) ? 1n : 0n;
        },
        dom_set_text(selector, value) {
            element(selector).textContent = titanString(value);
        },
        dom_set_html(selector, value) {
            element(selector).innerHTML = titanString(value);
        },
        dom_set_attribute(selector, name, value) {
            element(selector).setAttribute(titanString(name), titanString(value));
        },
        dom_add_class(selector, name) {
            element(selector).classList.add(titanString(name));
        },
        dom_remove_class(selector, name) {
            element(selector).classList.remove(titanString(name));
        },
        dom_focus(selector) {
            element(selector).focus();
        },
        dom_set_title(value) {
            document.title = titanString(value);
        },
        dom_listen(selector, eventName, handlerName) {
            const node = element(selector);
            const event = titanString(eventName);
            const exportedName = titanString(handlerName);
            const id = nextListenerId++;
            const callback = eventObject => {
                const previousEvent = currentEvent;
                currentEvent = eventObject;
                try {
                    const handler = instance.exports[exportedName];
                    if (typeof handler !== "function") {
                        throw new Error(`TITAN event handler is not exported: ${exportedName}`);
                    }
                    handler();
                } catch (error) {
                    console.error(`TITAN event handler '${exportedName}' failed`, error);
                } finally {
                    currentEvent = previousEvent;
                }
            };
            node.addEventListener(event, callback);
            listeners.set(id, { node, event, callback });
            return BigInt(id);
        },
        dom_unlisten(rawId) {
            const id = Number(rawId);
            if (!Number.isSafeInteger(id)) return 0n;
            const listener = listeners.get(id);
            if (!listener) return 0n;
            listener.node.removeEventListener(listener.event, listener.callback);
            listeners.delete(id);
            return 1n;
        },
        dom_event_type() {
            return eventString(eventObject => eventObject.type || "");
        },
        dom_event_value() {
            return eventString(eventObject =>
                typeof eventObject.target?.value === "string" ? eventObject.target.value : ""
            );
        },
        dom_event_key() {
            return eventString(eventObject => eventObject.key || "");
        },
        dom_event_target_id() {
            return eventString(eventObject => eventObject.target?.id || "");
        },
        dom_event_checked() {
            return currentEvent?.target?.checked === true ? 1n : 0n;
        },
        dom_event_x() {
            return BigInt(Math.trunc(Number(currentEvent?.clientX) || 0));
        },
        dom_event_y() {
            return BigInt(Math.trunc(Number(currentEvent?.clientY) || 0));
        },
        fetch_start(urlHandle, maximumHandle, timeoutHandle, handlerHandle) {
            const url = titanString(urlHandle);
            const maximumBytes = safeInteger(maximumHandle, "maximumBytes", 1);
            const timeoutMs = safeInteger(timeoutHandle, "timeoutMs", 0);
            const handlerName = titanString(handlerHandle);
            const id = nextFetchId++;
            const record = {
                controller: new AbortController(),
                cancelled: false,
                timedOut: false,
                timer: null,
            };
            if (timeoutMs > 0) {
                record.timer = setTimeout(() => {
                    record.timedOut = true;
                    record.controller.abort();
                }, timeoutMs);
            }
            requests.set(id, record);
            void performFetch(id, url, maximumBytes, handlerName, record);
            return BigInt(id);
        },
        fetch_cancel(rawId) {
            const id = safeInteger(rawId, "request id", 1);
            const record = requests.get(id);
            if (!record) return 0n;
            record.cancelled = true;
            record.controller.abort();
            if (record.timer !== null) clearTimeout(record.timer);
            requests.delete(id);
            return 1n;
        },
        fetch_ok() {
            return currentFetch?.ok === true ? 1n : 0n;
        },
        fetch_status() {
            return BigInt(currentFetch?.status || 0);
        },
        fetch_body() {
            return fetchString(context => context.body);
        },
        fetch_url() {
            return fetchString(context => context.url);
        },
        fetch_error() {
            return fetchString(context => context.error);
        },
    },
};

async function loadTitan(url) {
    const response = await fetch(url);
    if (!response.ok) throw new Error(`failed to load ${url}: HTTP ${response.status}`);
    const bytes = await response.arrayBuffer();
    const result = await WebAssembly.instantiate(bytes, imports);
    instance = result.instance;
    return instance.exports.main();
}

loadTitan("./program.wasm")
    .then(result => console.log("TITAN main returned", result))
    .catch(error => {
        console.error(error);
        document.querySelector("#status").textContent = `Error: ${error.message}`;
    });
