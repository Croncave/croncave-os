// A line diff (longest common subsequence) for reviewing an agent's changes.

export type DiffLine = { kind: 'same' | 'add' | 'del'; text: string; old?: number; new?: number };

export function diffLines(before: string, after: string): DiffLine[] {
	const a = before === '' ? [] : before.replace(/\n$/, '').split('\n');
	const b = after === '' ? [] : after.replace(/\n$/, '').split('\n');
	const n = a.length;
	const m = b.length;
	// Large files: fall back to "all removed, all added" rather than an O(n*m) table.
	if (n * m > 4_000_000) {
		return [...a.map((t, i) => ({ kind: 'del' as const, text: t, old: i + 1 })), ...b.map((t, i) => ({ kind: 'add' as const, text: t, new: i + 1 }))];
	}
	const lcs: Uint32Array[] = Array.from({ length: n + 1 }, () => new Uint32Array(m + 1));
	for (let i = n - 1; i >= 0; i--)
		for (let j = m - 1; j >= 0; j--) lcs[i][j] = a[i] === b[j] ? lcs[i + 1][j + 1] + 1 : Math.max(lcs[i + 1][j], lcs[i][j + 1]);
	const out: DiffLine[] = [];
	let i = 0;
	let j = 0;
	while (i < n && j < m) {
		if (a[i] === b[j]) {
			out.push({ kind: 'same', text: a[i], old: i + 1, new: j + 1 });
			i++;
			j++;
		} else if (lcs[i + 1][j] >= lcs[i][j + 1]) {
			out.push({ kind: 'del', text: a[i], old: ++i });
		} else {
			out.push({ kind: 'add', text: b[j], new: ++j });
		}
	}
	while (i < n) out.push({ kind: 'del', text: a[i], old: ++i });
	while (j < m) out.push({ kind: 'add', text: b[j], new: ++j });
	return out;
}

/** Keep `context` unchanged lines around each change; collapse the rest. */
export function hunks(lines: DiffLine[], context = 3): (DiffLine | { kind: 'gap'; count: number })[] {
	const keep = lines.map(() => false);
	lines.forEach((l, i) => {
		if (l.kind !== 'same') for (let k = Math.max(0, i - context); k <= Math.min(lines.length - 1, i + context); k++) keep[k] = true;
	});
	const out: (DiffLine | { kind: 'gap'; count: number })[] = [];
	let gap = 0;
	lines.forEach((l, i) => {
		if (keep[i]) {
			if (gap) out.push({ kind: 'gap', count: gap });
			gap = 0;
			out.push(l);
		} else gap++;
	});
	if (gap) out.push({ kind: 'gap', count: gap });
	return out;
}
