// Run the wasm32-wasip1 build of the layout spike under Node's WASI (V8, as in Chromium).
// Usage: node run-wasi.mjs <path-to-layout-spike.wasm> [args...]
import { readFile } from "node:fs/promises";
import { WASI } from "node:wasi";
import { argv } from "node:process";

const [wasmPath, ...args] = argv.slice(2);
const wasi = new WASI({ version: "preview1", args: ["layout-spike", ...args], env: {} });
const module = await WebAssembly.compile(await readFile(wasmPath));
const instance = await WebAssembly.instantiate(module, wasi.getImportObject());
wasi.start(instance);
