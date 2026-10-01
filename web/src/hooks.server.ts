import type { Handle } from '@sveltejs/kit';
import fallback from '../../crates/server/catalog/scheme-croncave.json';
import { schemeCss, type Scheme } from '$lib/theme/theme';

// The scheme is data served by the control plane; the bundled copy is only a fallback.
const API = process.env.API_URL ?? 'http://127.0.0.1:8080';
let cached = { at: 0, css: schemeCss(fallback as Scheme) };

async function themeCss(): Promise<string> {
	if (Date.now() - cached.at < 60_000) return cached.css;
	try {
		const r = await fetch(`${API}/api/schemes/croncave`);
		if (r.ok) cached = { at: Date.now(), css: schemeCss((await r.json()) as Scheme) };
	} catch {
		// Keep the last good scheme.
	}
	cached.at = Date.now();
	return cached.css;
}

// Applied before first paint: the page arrives with the person's mode and scheme set.
export const handle: Handle = async ({ event, resolve }) => {
	const saved = event.cookies.get('cc_mode');
	const mode = saved === 'light' || saved === 'system' ? saved : 'dark';
	const css = await themeCss();
	return resolve(event, {
		transformPageChunk: ({ html }) =>
			html.replace('%cc.mode%', mode).replace('%cc.theme%', `<style id="cc-theme">${css}</style>`)
	});
};
