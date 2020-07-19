const fs = require('fs');
const path = require('path');
const { WASI } = require('wasi');
const wasi = new WASI({
	args: process.argv,
	env: process.env,
	preopens: {},
});

class JsonnetVM {
	constructor(wasm, vm) {
		this.wasm = wasm;
		this.vm = vm;
		this.wasm.exports.jrsonnet_set_trace_format(this.vm, 1);

		this.setImportCallback((from, to) => {
			const resolved = path.resolve(from, to);
			return {
				value: fs.readFileSync(resolved).toString('utf-8'),
				foundHere: resolved,
			};
		})
	}

	/**
	 * @param {(from: string, to: string) => {foundHere: string, value: string}} cb
	 */
	setImportCallback(cb) {
		this.wasm.importCbs.set(this.vm, (base, rel, foundHere, success) => {
			const baseStr = this.wasm.readString(base);
			const relStr = this.wasm.readString(rel);
			try {
				const value = cb(baseStr, relStr);
				this.wasm.memorySlice32Len(foundHere, 1)[0] = this.allocateString(value.foundHere);
				this.wasm.memorySlice32Len(success, 1)[0] = 1;
				return this.allocateString(value.value);
			} catch (e) {
				this.wasm.memorySlice32Len(success, 1)[0] = 0;
				return this.allocateString(e.stack)
			}
		});
		this.wasm.exports.jrsonnet_apply_static_import_callback(
			this.vm,
			this.vm,
		);
	}

	alloc(length) {
		return this.wasm.exports.jsonnet_realloc(this.vm, 0, length);
	}
	allocateString(string) {
		const byteLength = new TextEncoder().encode(string).length;
		const addr = this.alloc(byteLength + 1);
		this.wasm.writeString(addr, string);
		return addr;
	}
	dealloc(addr) {
		return this.wasm.exports.jsonnet_realloc(this.vm, addr, 0);
	}

	evaluateFile(path) {
		const pathAddr = this.allocateString(path);
		const resultCodeAddr = this.alloc(4);
		const resultAddr = this.wasm.exports.jsonnet_evaluate_file(this.vm, pathAddr, resultCodeAddr);
		this.dealloc(pathAddr);
		const resultCode = this.wasm.memorySliceLen(resultCodeAddr, 4);
		this.dealloc(resultCodeAddr);
		const result = this.wasm.readString(resultAddr).trim();
		this.dealloc(resultAddr);
		if (resultCode[0] === 1) {
			const error = new Error(result);
			throw error;
		} else {
			return result;
		}
	}
	evaluateSnippet(path, snippet) {
		const pathAddr = this.allocateString(path);
		const snippetAddr = this.allocateString(snippet);
		const resultCodeAddr = this.alloc(4);
		const resultAddr = this.wasm.exports.jsonnet_evaluate_snippet(this.vm, pathAddr, snippetAddr, resultCodeAddr);
		this.dealloc(pathAddr);
		this.dealloc(snippetAddr);
		const resultCode = this.wasm.memorySliceLen(resultCodeAddr, 4);
		this.dealloc(resultCodeAddr);
		const result = this.wasm.readString(resultAddr);
		this.dealloc(resultAddr);
		if (resultCode[0] === 1) {
			const error = new Error(result);
			throw error;
		} else {
			return result;
		}
	}

	/**
	 * Destroys vm, any future call to this object will fail, and all resources will be freed
	 */
	destroy() {
		this.wasm.exports.jsonnet_destroy(this.vm);
		this.wasm.importCbs.delete(this.vm);
	}
}

class JsonnetWASM {
	constructor() {
		this.importCbs = new Map();
	}

	async init(buf) {
		const wasm = await WebAssembly.compile(buf);
		const instance = await WebAssembly.instantiate(wasm, {
			wasi_snapshot_preview1: wasi.wasiImport,
			env: {
				_jrsonnet_static_import_callback: (ctx, base, rel, found_here, success) => {
					if (!this.importCbs.has(ctx)) {
						throw new Error(`Got unknown ctx callback: ${ctx}`);
					}
					return this.importCbs.get(ctx)(base, rel, found_here, success);
				}
			}
		});
		wasi.start(instance);
		this.instance = instance;
	}
	/**
	 * @type Record<string, WebAssembly.ExportValue>
	 */
	get exports() {
		return this.instance.exports;
	}
	get memory() {
		return this.exports.memory;
	}
	get memoryBuffer() {
		return this.memory.buffer;
	}
	memorySliceLen(start, length) {
		return new Uint8Array(this.memoryBuffer, start, length);
	}
	memorySlice32Len(start, length) {
		return new Uint32Array(this.memoryBuffer, start, length);
	}
	memorySlice(start, end) {
		return new Uint8Array(this.memoryBuffer, start, start && end && (end - start));
	}

	readString(addr) {
		let end;
		let slice = this.memorySlice();
		for (end = addr; slice[end]; end++);
		return (new TextDecoder()).decode(this.memorySlice(addr, end));
	}
	writeString(addr, string) {
		let slice = this.memorySlice(addr);
		let result = new TextEncoder().encodeInto(string, slice);
		slice[result.written] = 0;
	}

	version() {
		return this.readString(this.exports.jsonnet_version());
	}

	newVM() {
		return new JsonnetVM(this, this.exports.jsonnet_make());
	}
}

(async () => {
	try {
		const jsonnet = new JsonnetWASM();
		await jsonnet.init(fs.readFileSync(`${__dirname}/../../target/wasm32-wasi/release/jsonnet.wasm`));
		console.log(`Version = ${jsonnet.version()}`);

		const vm = jsonnet.newVM();
		console.log(vm.evaluateSnippet('./snip.jsonnet', `
			2+2
		`));
		console.log(vm.evaluateFile('./test.jsonnet'));
	} catch (e) {
		console.log(e.stack);
	}
})();
