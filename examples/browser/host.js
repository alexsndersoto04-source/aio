const decoder = new TextDecoder("utf-8", { fatal: true });
let instance;

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

function element(handle) {
    const selector = titanString(handle);
    const node = document.querySelector(selector);
    if (!node) throw new Error(`DOM selector did not match: ${selector}`);
    return node;
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
