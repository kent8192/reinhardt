// WASM SPA initialization script.
// Loaded as an external module to comply with Content Security Policy
// (script-src: 'self' 'wasm-unsafe-eval') without requiring 'unsafe-inline'.
//
// The import path is rewritten at runtime by the admin SPA HTML shell
// (see router.rs::admin_spa_html), so this file is a static template
// whose actual entry point URL is resolved via collectstatic manifests.

const scriptEl = document.querySelector('script[data-wasm-entry]');
const entryUrl = scriptEl?.dataset.wasmEntry;
if (entryUrl) {
	const { default: init } = await import(entryUrl);
	await init();
} else {
	console.error('reinhardt-admin: missing data-wasm-entry attribute on init script');
}
