import { describe, expect, it } from 'vitest';
import croncave from '../../../../crates/server/catalog/scheme-croncave.json';
import { contrast, schemeCss, schemeProblems, type Scheme } from './theme';

describe('color schemes', () => {
	it('the Croncave scheme passes every check in both modes', () => {
		expect(schemeProblems(croncave as Scheme)).toEqual([]);
	});

	it('contrast follows WCAG', () => {
		expect(contrast('#000000', '#ffffff')).toBeCloseTo(21, 0);
		expect(contrast('#777777', '#777777')).toBeCloseTo(1, 5);
	});

	it('catches a scheme with unreadable text or confusable statuses', () => {
		const bad = structuredClone(croncave) as Scheme;
		bad.tokens.mid.dark = bad.tokens.surface.dark;
		bad.tokens.failed.light = bad.tokens.needs.light;
		const p = schemeProblems(bad);
		expect(p.some((m) => m.includes('mid on surface'))).toBe(true);
		expect(p.some((m) => m.includes('look too alike'))).toBe(true);
	});

	it('generates dark, light and match-system rules', () => {
		const css = schemeCss(croncave as Scheme);
		expect(css).toContain(':root[data-mode=light]{');
		expect(css).toContain('prefers-color-scheme: light');
		expect(css).toContain('--accent:');
	});
});
