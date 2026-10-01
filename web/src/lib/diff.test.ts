import { describe, expect, it } from 'vitest';
import { diffLines, hunks } from './diff';

describe('line diff', () => {
	it('finds added, removed and kept lines', () => {
		const d = diffLines('a\nb\nc\n', 'a\nB\nc\nd\n');
		expect(d.map((l) => `${l.kind}:${l.text}`)).toEqual(['same:a', 'del:b', 'add:B', 'same:c', 'add:d']);
	});

	it('treats a new file as all added', () => {
		expect(diffLines('', 'x\ny').every((l) => l.kind === 'add')).toBe(true);
	});

	it('collapses unchanged runs into gaps', () => {
		const before = Array.from({ length: 20 }, (_, i) => `l${i}`).join('\n');
		const after = before.replace('l10', 'L10');
		const h = hunks(diffLines(before, after), 2);
		expect(h[0]).toEqual({ kind: 'gap', count: 8 });
		expect(h.filter((l) => l.kind !== 'gap').length).toBe(6);
	});
});
