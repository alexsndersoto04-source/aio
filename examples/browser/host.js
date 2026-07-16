const decoder = new TextDecoder("utf-8", { fatal: true });
const responseDecoder = new TextDecoder("utf-8");
const encoder = new TextEncoder();
let instance;
let currentEvent = null;
let currentFetch = null;
let currentSocket = null;
let currentFrame = null;
let nextListenerId = 1;
let nextFetchId = 1;
let nextSocketId = 1;
let nextAnimationId = 1;
const listeners = new Map();
const requests = new Map();
const sockets = new Map();
const animations = new Map();

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

function signedInteger(raw, name) {
    const value = typeof raw === "bigint" ? raw : BigInt(raw);
    const limit = BigInt(Number.MAX_SAFE_INTEGER);
    if (value < -limit || value > limit) throw new RangeError(`${name} is outside the supported range`);
    return Number(value);
}

function canvasContext(selectorHandle) {
    const canvas = element(selectorHandle);
    if (!(canvas instanceof HTMLCanvasElement)) throw new TypeError("selector does not identify a canvas");
    const context = canvas.getContext("2d");
    if (!context) throw new Error("Canvas 2D context is unavailable");
    return { canvas, context };
}

function lineWidth(raw) {
    const width = safeInteger(raw, "line width", 1);
    if (width > 10_000) throw new RangeError("line width exceeds 10000");
    return width;
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

function parseRequestHeaders(source) {
    if (source.trim() === "") return new Headers();
    const parsed = JSON.parse(source);
    if (parsed === null || Array.isArray(parsed) || typeof parsed !== "object") {
        throw new TypeError("request headers must be a JSON object");
    }
    const headers = new Headers();
    for (const [name, value] of Object.entries(parsed)) {
        if (typeof value !== "string") throw new TypeError(`header '${name}' must be a string`);
        if (name.includes("\r") || name.includes("\n") || value.includes("\r") || value.includes("\n")) {
            throw new TypeError("request headers cannot contain CR or LF");
        }
        headers.set(name, value);
    }
    return headers;
}

function requestOptions(methodSource, headersSource, body) {
    const method = methodSource.trim().toUpperCase();
    if (!/^[!#$%&'*+\-.^_`|~0-9A-Z]+$/.test(method)) {
        throw new TypeError("invalid HTTP method");
    }
    const options = { method, headers: parseRequestHeaders(headersSource) };
    if (method === "GET" || method === "HEAD") {
        if (body !== "") throw new TypeError(`${method} requests cannot have a body`);
    } else if (body !== "") {
        options.body = body;
    }
    return options;
}

function startFetch(url, maximumBytes, timeoutMs, handlerName, options) {
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
    void performFetch(id, url, maximumBytes, handlerName, record, options);
    return BigInt(id);
}

async function performFetch(id, url, maximumBytes, handlerName, record, options) {
    try {
        const response = await globalThis.fetch(url, {
            ...options,
            signal: record.controller.signal,
        });
        const body = await readBoundedBody(response, maximumBytes);
        if (!record.cancelled) {
            invokeFetchHandler(handlerName, {
                ok: response.ok,
                status: response.status,
                body,
                url: response.url,
                error: "",
                headers: JSON.stringify(Object.fromEntries(response.headers.entries())),
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
                headers: "{}",
            });
        }
    } finally {
        if (record.timer !== null) clearTimeout(record.timer);
        if (requests.get(id) === record) requests.delete(id);
    }
}

function socketString(read) {
    return wasmString(currentSocket ? read(currentSocket) : "");
}

function parseProtocols(source) {
    if (source.trim() === "") return [];
    const protocols = JSON.parse(source);
    if (!Array.isArray(protocols) || protocols.length > 32) {
        throw new TypeError("WebSocket protocols must be a JSON array with at most 32 entries");
    }
    const seen = new Set();
    for (const protocol of protocols) {
        if (typeof protocol !== "string" || protocol === "" || encoder.encode(protocol).length > 128) {
            throw new TypeError("invalid WebSocket subprotocol");
        }
        if (seen.has(protocol)) throw new TypeError(`duplicate WebSocket subprotocol: ${protocol}`);
        seen.add(protocol);
    }
    return protocols;
}

function invokeSocketHandler(handlerName, context) {
    const previousSocket = currentSocket;
    currentSocket = context;
    try {
        const handler = instance.exports[handlerName];
        if (typeof handler !== "function") {
            throw new Error(`TITAN WebSocket handler is not exported: ${handlerName}`);
        }
        handler();
    } catch (error) {
        console.error(`TITAN WebSocket handler '${handlerName}' failed`, error);
    } finally {
        currentSocket = previousSocket;
    }
}

function socketContext(id, record, values = {}) {
    return {
        id,
        message: "",
        protocol: record.socket.protocol || "",
        closeCode: 0,
        closeReason: "",
        wasClean: false,
        error: "",
        ...values,
    };
}

function runAnimationFrame(id, record, timestamp) {
    if (!record.active) return;
    record.count += 1;
    const delta = record.previousTime === null ? 0 : Math.max(0, timestamp - record.previousTime);
    record.previousTime = timestamp;
    const previousFrame = currentFrame;
    currentFrame = {
        id,
        time: Math.trunc(timestamp),
        delta: Math.trunc(delta),
        count: record.count,
    };
    let succeeded = true;
    try {
        record.handler();
    } catch (error) {
        succeeded = false;
        console.error(`TITAN animation handler '${record.handlerName}' failed`, error);
    } finally {
        currentFrame = previousFrame;
    }
    if (record.active && succeeded) {
        record.request = requestAnimationFrame(nextTimestamp => runAnimationFrame(id, record, nextTimestamp));
    } else {
        record.active = false;
        animations.delete(id);
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
            return startFetch(
                titanString(urlHandle),
                safeInteger(maximumHandle, "maximumBytes", 1),
                safeInteger(timeoutHandle, "timeoutMs", 0),
                titanString(handlerHandle),
                { method: "GET" }
            );
        },
        fetch_request(methodHandle, urlHandle, headersHandle, bodyHandle, maximumHandle, timeoutHandle, handlerHandle) {
            const method = titanString(methodHandle);
            const headers = titanString(headersHandle);
            const body = titanString(bodyHandle);
            return startFetch(
                titanString(urlHandle),
                safeInteger(maximumHandle, "maximumBytes", 1),
                safeInteger(timeoutHandle, "timeoutMs", 0),
                titanString(handlerHandle),
                requestOptions(method, headers, body)
            );
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
        fetch_headers() {
            return fetchString(context => context.headers);
        },
        ws_connect(urlHandle, protocolsHandle, maximumHandle, openHandle, messageHandle, errorHandle, closeHandle) {
            const url = titanString(urlHandle);
            const protocols = parseProtocols(titanString(protocolsHandle));
            const maximumBytes = safeInteger(maximumHandle, "maximumMessageBytes", 1);
            const handlers = {
                open: titanString(openHandle),
                message: titanString(messageHandle),
                error: titanString(errorHandle),
                close: titanString(closeHandle),
            };
            const id = nextSocketId++;
            const socket = protocols.length === 0 ? new WebSocket(url) : new WebSocket(url, protocols);
            socket.binaryType = "arraybuffer";
            const record = { socket, maximumBytes, handlers };
            sockets.set(id, record);
            socket.addEventListener("open", () => {
                invokeSocketHandler(handlers.open, socketContext(id, record));
            });
            socket.addEventListener("message", event => {
                let message;
                let length;
                if (typeof event.data === "string") {
                    message = event.data;
                    length = encoder.encode(message).length;
                } else if (event.data instanceof ArrayBuffer) {
                    length = event.data.byteLength;
                    message = responseDecoder.decode(new Uint8Array(event.data));
                } else {
                    invokeSocketHandler(handlers.error, socketContext(id, record, {
                        error: "unsupported WebSocket message type",
                    }));
                    socket.close(1003, "unsupported message type");
                    return;
                }
                if (length > maximumBytes) {
                    invokeSocketHandler(handlers.error, socketContext(id, record, {
                        error: "WebSocket message exceeds maximumMessageBytes",
                    }));
                    socket.close(1009, "message too large");
                    return;
                }
                invokeSocketHandler(handlers.message, socketContext(id, record, { message }));
            });
            socket.addEventListener("error", () => {
                invokeSocketHandler(handlers.error, socketContext(id, record, {
                    error: "WebSocket transport error",
                }));
            });
            socket.addEventListener("close", event => {
                sockets.delete(id);
                invokeSocketHandler(handlers.close, socketContext(id, record, {
                    closeCode: event.code,
                    closeReason: event.reason,
                    wasClean: event.wasClean,
                }));
            });
            return BigInt(id);
        },
        ws_send(rawId, messageHandle) {
            const id = safeInteger(rawId, "WebSocket id", 1);
            const record = sockets.get(id);
            if (!record || record.socket.readyState !== WebSocket.OPEN) return 0n;
            const message = titanString(messageHandle);
            const messageBytes = encoder.encode(message).length;
            if (messageBytes > record.maximumBytes) return 0n;
            if (record.socket.bufferedAmount + messageBytes > record.maximumBytes) return 0n;
            record.socket.send(message);
            return 1n;
        },
        ws_close(rawId, rawCode, reasonHandle) {
            const id = safeInteger(rawId, "WebSocket id", 1);
            const code = safeInteger(rawCode, "WebSocket close code", 0);
            const reason = titanString(reasonHandle);
            const record = sockets.get(id);
            if (!record || record.socket.readyState >= WebSocket.CLOSING) return 0n;
            if (code !== 1000 && (code < 3000 || code > 4999)) return 0n;
            if (encoder.encode(reason).length > 123) return 0n;
            record.socket.close(code, reason);
            return 1n;
        },
        ws_id() {
            return BigInt(currentSocket?.id || 0);
        },
        ws_message() {
            return socketString(context => context.message);
        },
        ws_protocol() {
            return socketString(context => context.protocol);
        },
        ws_close_code() {
            return BigInt(currentSocket?.closeCode || 0);
        },
        ws_close_reason() {
            return socketString(context => context.closeReason);
        },
        ws_was_clean() {
            return currentSocket?.wasClean === true ? 1n : 0n;
        },
        ws_error() {
            return socketString(context => context.error);
        },
        canvas_resize(selector, rawWidth, rawHeight) {
            const { canvas } = canvasContext(selector);
            const width = safeInteger(rawWidth, "canvas width", 1);
            const height = safeInteger(rawHeight, "canvas height", 1);
            if (width > 16_384 || height > 16_384) throw new RangeError("canvas dimensions exceed 16384");
            if (width * height > 67_108_864) throw new RangeError("canvas pixel area exceeds 67108864");
            canvas.width = width;
            canvas.height = height;
        },
        canvas_clear(selector, colorHandle) {
            const { canvas, context } = canvasContext(selector);
            const color = titanString(colorHandle);
            context.clearRect(0, 0, canvas.width, canvas.height);
            if (color !== "") {
                context.save();
                context.fillStyle = color;
                context.fillRect(0, 0, canvas.width, canvas.height);
                context.restore();
            }
        },
        canvas_fill_rect(selector, rawX, rawY, rawWidth, rawHeight, colorHandle) {
            const { context } = canvasContext(selector);
            context.save();
            context.fillStyle = titanString(colorHandle);
            context.fillRect(
                signedInteger(rawX, "rectangle x"),
                signedInteger(rawY, "rectangle y"),
                signedInteger(rawWidth, "rectangle width"),
                signedInteger(rawHeight, "rectangle height")
            );
            context.restore();
        },
        canvas_stroke_rect(selector, rawX, rawY, rawWidth, rawHeight, colorHandle, rawLineWidth) {
            const { context } = canvasContext(selector);
            context.save();
            context.strokeStyle = titanString(colorHandle);
            context.lineWidth = lineWidth(rawLineWidth);
            context.strokeRect(
                signedInteger(rawX, "rectangle x"),
                signedInteger(rawY, "rectangle y"),
                signedInteger(rawWidth, "rectangle width"),
                signedInteger(rawHeight, "rectangle height")
            );
            context.restore();
        },
        canvas_line(selector, rawX1, rawY1, rawX2, rawY2, colorHandle, rawLineWidth) {
            const { context } = canvasContext(selector);
            context.save();
            context.strokeStyle = titanString(colorHandle);
            context.lineWidth = lineWidth(rawLineWidth);
            context.beginPath();
            context.moveTo(signedInteger(rawX1, "line x1"), signedInteger(rawY1, "line y1"));
            context.lineTo(signedInteger(rawX2, "line x2"), signedInteger(rawY2, "line y2"));
            context.stroke();
            context.restore();
        },
        canvas_text(selector, textHandle, rawX, rawY, colorHandle, fontHandle) {
            const { context } = canvasContext(selector);
            context.save();
            context.fillStyle = titanString(colorHandle);
            context.font = titanString(fontHandle);
            context.fillText(
                titanString(textHandle),
                signedInteger(rawX, "text x"),
                signedInteger(rawY, "text y")
            );
            context.restore();
        },
        animation_start(handlerHandle) {
            const handlerName = titanString(handlerHandle);
            const handler = instance.exports[handlerName];
            if (typeof handler !== "function") {
                throw new Error(`TITAN animation handler is not exported: ${handlerName}`);
            }
            const id = nextAnimationId++;
            const record = {
                active: true,
                request: 0,
                previousTime: null,
                count: 0,
                handler,
                handlerName,
            };
            animations.set(id, record);
            record.request = requestAnimationFrame(timestamp => runAnimationFrame(id, record, timestamp));
            return BigInt(id);
        },
        animation_cancel(rawId) {
            const id = safeInteger(rawId, "animation id", 1);
            const record = animations.get(id);
            if (!record) return 0n;
            record.active = false;
            cancelAnimationFrame(record.request);
            animations.delete(id);
            return 1n;
        },
        frame_id() {
            return BigInt(currentFrame?.id || 0);
        },
        frame_time_ms() {
            return BigInt(currentFrame?.time || 0);
        },
        frame_delta_ms() {
            return BigInt(currentFrame?.delta || 0);
        },
        frame_count() {
            return BigInt(currentFrame?.count || 0);
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
