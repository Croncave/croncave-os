// A color scheme is data: light and dark values for each semantic token. Components only
// ever use var(--token); this turns a scheme into the CSS that defines them.

export type Scheme = {
	id: string;
	name: string;
	version: number;
	tokens: Record<string, { dark: string; light: string }>;
};

export type Mode = 'dark' | 'light' | 'system';

function block(scheme: Scheme, mode: 'dark' | 'light'): string {
	return Object.entries(scheme.tokens)
		.map(([name, v]) => `--${name}:${v[mode]};`)
		.join('');
}

/** CSS for a scheme: dark by default, light on request, and "match system". */
export function schemeCss(scheme: Scheme): string {
	const dark = block(scheme, 'dark');
	const light = block(scheme, 'light');
	return (
		`:root,:root[data-mode=dark]{${dark}color-scheme:dark}` +
		`:root[data-mode=light]{${light}color-scheme:light}` +
		`@media (prefers-color-scheme: light){:root[data-mode=system]{${light}color-scheme:light}}`
	);
}

// ---------------------------------------------------------------------------------------
// Checks a scheme must pass before it ships, in both modes.
// ---------------------------------------------------------------------------------------

function rgb(hex: string): [number, number, number] | null {
	const m = /^#([0-9a-f]{6})$/i.exec(hex.trim());
	if (!m) return null;
	const n = parseInt(m[1], 16);
	return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

function luminance([r, g, b]: [number, number, number]): number {
	const f = (c: number) => {
		const s = c / 255;
		return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
	};
	return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b);
}

export function contrast(a: string, b: string): number {
	const x = rgb(a);
	const y = rgb(b);
	if (!x || !y) return 0;
	const [l1, l2] = [luminance(x), luminance(y)].sort((p, q) => q - p);
	return (l1 + 0.05) / (l2 + 0.05);
}

function lab([r, g, b]: [number, number, number]): [number, number, number] {
	const lin = (c: number) => {
		const s = c / 255;
		return s <= 0.04045 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
	};
	const [R, G, B] = [lin(r), lin(g), lin(b)];
	const x = (0.4124 * R + 0.3576 * G + 0.1805 * B) / 0.95047;
	const y = 0.2126 * R + 0.7152 * G + 0.0722 * B;
	const z = (0.0193 * R + 0.1192 * G + 0.9505 * B) / 1.08883;
	const f = (t: number) => (t > 216 / 24389 ? Math.cbrt(t) : (24389 / 27 * t + 16) / 116);
	return [116 * f(y) - 16, 500 * (f(x) - f(y)), 200 * (f(y) - f(z))];
}

/** Perceptual difference (CIE76 ΔE). Around 20 or more reads as clearly different. */
export function deltaE(a: string, b: string): number {
	const x = rgb(a);
	const y = rgb(b);
	if (!x || !y) return 0;
	const [p, q] = [lab(x), lab(y)];
	return Math.hypot(p[0] - q[0], p[1] - q[1], p[2] - q[2]);
}

const DISTINCT = 20;

const TEXT = ['ink', 'mid', 'low'];
const BACKGROUNDS = ['bg', 'surface', 'pane'];
const STATUS = ['live', 'working', 'needs', 'failed', 'asleep'];
const REQUIRED = [...TEXT, ...BACKGROUNDS, ...STATUS, 'line', 'accent', 'on-accent', 'accent-ink', 'accent-soft', 'focus'];

/** Every problem with a scheme, in plain words. Empty means it can ship. */
export function schemeProblems(scheme: Scheme): string[] {
	const out: string[] = [];
	for (const t of REQUIRED) if (!scheme.tokens[t]) out.push(`The scheme has no "${t}" color.`);
	if (out.length) return out;
	for (const mode of ['dark', 'light'] as const) {
		const c = (t: string) => scheme.tokens[t][mode];
		for (const text of TEXT)
			for (const bg of BACKGROUNDS)
				if (contrast(c(text), c(bg)) < 4.5)
					out.push(`${mode}: ${text} on ${bg} is ${contrast(c(text), c(bg)).toFixed(2)}:1; text needs 4.5:1.`);
		if (contrast(c('on-accent'), c('accent')) < 4.5) out.push(`${mode}: text on the accent needs 4.5:1.`);
		if (contrast(c('accent-ink'), c('surface')) < 4.5) out.push(`${mode}: links need 4.5:1 on surfaces.`);
		for (const t of ['accent', 'focus'])
			for (const bg of BACKGROUNDS)
				if (contrast(c(t), c(bg)) < 3) out.push(`${mode}: ${t} on ${bg} needs 3:1 (controls and focus rings).`);
		for (const s of STATUS) {
			// A status word is drawn in its color.
			if (contrast(c(s), c('surface')) < 4.5) out.push(`${mode}: the "${s}" status needs 4.5:1 on surfaces.`);
		}
		for (let i = 0; i < STATUS.length; i++)
			for (let j = i + 1; j < STATUS.length; j++) {
				const [a, b] = [STATUS[i], STATUS[j]];
				// "live" may share the accent's ink; every other pair must look different.
				if (deltaE(c(a), c(b)) < DISTINCT && !(a === 'live' || b === 'live'))
					out.push(`${mode}: "${a}" and "${b}" look too alike.`);
			}
		for (const s of STATUS.filter((s) => s !== 'live'))
			if (deltaE(c(s), c('accent')) < DISTINCT) out.push(`${mode}: "${s}" looks too much like the accent.`);
	}
	return out;
}
