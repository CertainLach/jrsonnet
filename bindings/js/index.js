const fs = require('fs');
const { WASI } = require('wasi');
const wasi = new WASI({
	args: process.argv,
	env: process.env,
	preopens: {},
});
const importObject = { wasi_snapshot_preview1: wasi.wasiImport };

class JsonnetVM {
	constructor(wasm, vm) {
		this.wasm = wasm;
		this.vm = vm;
	}

	alloc(length) {
		return this.wasm.exports.jsonnet_realloc(this.vm, 0, length);
	}
	allocateString(string) {
		const byteLength = new TextEncoder().encode(string).length;
		const addr = this.alloc(byteLength);
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
			const error = new Error(this.normalizeErrorString(result));
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
			const error = new Error(this.normalizeErrorString(result));
			throw error;
		} else {
			return result;
		}
	}
	normalizeErrorString(str) {
		str = str.trim();
		const newLine = str.indexOf('\n');
		if (newLine === -1) return str;
		let message = str.slice(0, newLine);
		let trace = str.slice(newLine + 1).split('\n').map(s => s.split(' ---- ')).map(([p, v]) => `    at ${v} (${p})`).join('\n');
		return `${message}\n${trace}`;
	}
}

class JsonnetWASM {
	constructor() { }

	async init(buf) {
		const wasm = await WebAssembly.compile(buf);
		const instance = await WebAssembly.instantiate(wasm, importObject);
		wasi.start(instance);
		this.instance = instance;
	}
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
		await jsonnet.init(fs.readFileSync(`${__dirname}/../../target/wasm32-wasi/release/jsonnet.wasi.wasm`));
		console.log(`Version = ${jsonnet.version()}`);

		const vm = jsonnet.newVM();
		console.log(vm.evaluateSnippet('./snip.jsonnet', `
			local a(b) = error "sad" + b;
			local c() = a(2 + 2);
			c()
		`))
		console.log(vm.evaluateFile('./test.jsonnet'));
	} catch (e) {
		console.log(e.stack);
	}
})();
