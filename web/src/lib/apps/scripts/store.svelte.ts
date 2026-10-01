import { get } from '$lib/api';
import { session } from '$lib/session.svelte';
import type { Job } from '$lib/types';

// The scripts on the current computer, shared by the Scripts list and its pages.
export const scripts = $state({ jobs: [] as Job[], loaded: false, minSecs: 60, computer: '' });

export async function loadScripts() {
	const c = session.computerId;
	if (!c) return;
	const r = await get(`/computers/${c}/jobs?app=scripts`);
	scripts.jobs = r.jobs;
	scripts.computer = c;
	scripts.loaded = true;
	try {
		scripts.minSecs = (await get('/billing')).effective_plan.min_schedule_secs;
	} catch {
		// Keep the last known limit.
	}
}

export const lang = (path: string) => ({ py: 'PY', js: 'JS', mjs: 'JS', cjs: 'JS', sh: 'SH', bash: 'SH' })[path.split('.').pop()?.toLowerCase() ?? ''] ?? '';

export function runtimeOf(path: string, runtime?: string | null): 'python' | 'node' | 'shell' | '' {
	if (runtime === 'python' || runtime === 'node' || runtime === 'shell') return runtime;
	const l = lang(path);
	return l === 'PY' ? 'python' : l === 'JS' ? 'node' : l === 'SH' ? 'shell' : '';
}

/** "Python 3.12", from the versions the computer reports. */
export function runtimeName(rt: string, versions: Record<string, string> = {}) {
	const short = (v?: string) => (v ? ' ' + v.split('.').slice(0, 2).join('.') : '');
	if (rt === 'python') return `Python${short(versions.python)}`;
	if (rt === 'node') return `Node.js${versions.node ? ' ' + versions.node.split('.')[0] : ''}`;
	if (rt === 'shell') return `Bash${short(versions.bash)}`;
	return 'Script';
}

const ACTIVE = ['queued', 'waiting', 'starting', 'running'];

/** The pill a script shows: what it's doing, or how it's set to run. */
export function scriptState(j: Job): { status: string; word: string } {
	const r = j.last_run;
	if (r && ACTIVE.includes(r.status)) {
		const p = (r as { progress?: { done?: number; total?: number } | null }).progress;
		const pct = p?.total ? ` · ${Math.round(((p.done ?? 0) / p.total) * 100)}%` : '';
		return r.status === 'running' ? { status: 'running', word: `Running${pct}` } : { status: 'waiting', word: 'Waiting to run' };
	}
	if (j.status === 'paused') return { status: 'stopped', word: 'Paused' };
	if (r && ['failed', 'timed_out'].includes(r.status)) return { status: 'failed', word: 'Failed' };
	if (j.trigger === 'files') return { status: 'asleep', word: 'Waiting for files' };
	if (j.trigger === 'schedule') return { status: 'asleep', word: 'Scheduled' };
	return { status: 'asleep', word: 'By hand' };
}

export const isActive = (status: string) => ACTIVE.includes(status);

/** The packages a script needs, from the requirements.txt or package.json beside it. */
export async function findPackages(computer: string, path: string, rt: string): Promise<{ names: string[]; from: string } | null> {
	const manifest = rt === 'python' ? 'requirements.txt' : rt === 'node' ? 'package.json' : '';
	if (!manifest || !path) return null;
	const dir = path.split('/').slice(0, -1).join('/');
	try {
		const r = await get(`/computers/${computer}/files/preview?path=${encodeURIComponent(dir ? `${dir}/${manifest}` : manifest)}`);
		if (r.kind !== 'text') return null;
		if (manifest === 'package.json') {
			try {
				return { names: Object.keys(JSON.parse(r.text).dependencies ?? {}), from: manifest };
			} catch {
				return { names: [], from: manifest };
			}
		}
		const names = r.text
			.split('\n')
			.map((l: string) => l.replace(/#.*/, '').trim().split(/[<>=~![ ;]/)[0])
			.filter(Boolean);
		return { names, from: manifest };
	} catch {
		return null;
	}
}
