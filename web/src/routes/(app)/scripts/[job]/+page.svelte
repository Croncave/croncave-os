<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { del, get, patch, post, message } from '$lib/api';
	import { onChange, onLive, throttle } from '$lib/live';
	import { session } from '$lib/session.svelte';
	import { bytes, clock, dayAndTime, span, timeOfDay } from '$lib/format';
	import type { Job, RunBrief } from '$lib/types';
	import Button from '$lib/ui/Button.svelte';
	import Icon from '$lib/ui/Icon.svelte';
	import Status from '$lib/ui/Status.svelte';
	import { toast } from '$lib/ui/toast.svelte';
	import Output from '$lib/apps/scripts/Output.svelte';
	import RunView from '$lib/apps/scripts/RunView.svelte';
	import RunsChart from '$lib/apps/scripts/RunsChart.svelte';
	import ScriptAdd from '$lib/apps/scripts/ScriptAdd.svelte';
	import { findPackages, isActive, loadScripts, runtimeName, runtimeOf, scriptState } from '$lib/apps/scripts/store.svelte';

	// One script: what it does and when (Overview), each run (Runs), its source (Code) and
	// how it's set up (Settings).
	const id = $derived(page.params.job ?? '');
	const tab = $derived(page.url.searchParams.get('tab') ?? 'overview');
	const runParam = $derived(page.url.searchParams.get('run'));

	let job = $state<Job | null>(null);
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	let run = $state<any>(null);
	let source = $state<string[] | null>(null);
	let packages = $state<{ names: string[]; from: string } | null>(null);

	async function load() {
		try {
			job = await get(`/jobs/${id}`);
		} catch (e) {
			toast(message(e), true);
		}
	}
	$effect(() => {
		void id;
		job = null;
		load();
	});
	$effect(() => onChange(throttle(load, 400)));

	const runs = $derived<RunBrief[]>((job?.runs ?? []).filter((r) => r.trigger !== 'test'));
	const runId = $derived(runParam ?? runs[0]?.id ?? '');
	async function loadRun() {
		const want = runId;
		if (!want || tab !== 'runs') return;
		const r = await get(`/runs/${want}`);
		if (want === runId) run = r;
	}
	$effect(() => {
		void runId;
		void tab;
		loadRun();
	});
	$effect(() => onChange(throttle(loadRun, 400)));
	// Progress (step counts, CPU and memory) arrives every few seconds while a run goes.
	$effect(() => {
		const refresh = throttle(loadRun, 1000);
		return onLive((m) => m.kind === 'progress' && m.id === runId && refresh());
	});
	// The header's "running for" keeps counting.
	let now = $state(Date.now());
	$effect(() => {
		const t = setInterval(() => (now = Date.now()), 1000);
		return () => clearInterval(t);
	});

	const rt = $derived(job ? runtimeOf(job.setup.path, job.setup.runtime) : '');
	const c = $derived(session.me?.computers.find((x) => x.id === job?.computer_id));
	const versions = $derived<Record<string, string>>((c?.health as { runtimes?: Record<string, string> })?.runtimes ?? {});
	const file = $derived(job?.setup.path.split('/').pop() ?? '');
	const folder = $derived(job?.setup.path.split('/').slice(0, -1).join('/') ?? '');
	const st = $derived(job ? scriptState(job) : null);
	const last = $derived(job?.last_run ?? null);
	const going = $derived(!!last && isActive(last.status));

	let loadedSource = '';
	$effect(() => {
		const j = job;
		if (!j || (tab !== 'code' && tab !== 'overview')) return;
		const key = `${j.computer_id}:${j.setup.path}`;
		if (key === loadedSource) return;
		loadedSource = key;
		get(`/computers/${j.computer_id}/files/preview?path=${encodeURIComponent(j.setup.path)}`)
			.then((r) => (source = r.kind === 'text' ? r.text.replace(/\n$/, '').split('\n') : null))
			.catch(() => (source = null));
		findPackages(j.computer_id, j.setup.path, runtimeOf(j.setup.path, j.setup.runtime)).then((r) => (packages = r));
	});

	// "Next run at 3:00 PM, in 1 h 12 min" / "Started 4:12 AM by you · running for 2 h" / "failed 2 times in a row".
	const sub = $derived.by(() => {
		if (!job) return '';
		const parts = [runtimeName(rt, versions)];
		if (last && going) {
			parts.push(`Started ${timeOfDay(last.started_at ?? last.queued_at)}${last.started_by === 'user' ? ' by you' : ''}`);
			if (last.started_at) parts.push(`running for ${span((now - new Date(last.started_at).getTime()) / 1000)}`);
		} else if (last && ['failed', 'timed_out'].includes(last.status)) {
			let n = 0;
			for (const r of runs) {
				if (!['failed', 'timed_out'].includes(r.status)) break;
				n++;
			}
			parts.push(`Last ran ${dayAndTime(last.ended_at ?? last.queued_at)}`);
			if (n > 1) parts.push(`failed ${n} times in a row`);
		} else if (job.status === 'paused') {
			parts.push('Paused');
		} else if (job.next_due_at) {
			parts.push(`Next run at ${timeOfDay(job.next_due_at)}, in ${span((new Date(job.next_due_at).getTime() - now) / 1000)}`);
		} else if (last?.ended_at) {
			parts.push(`Last ran ${dayAndTime(last.ended_at)}`);
		} else {
			parts.push('Not run yet');
		}
		return parts.join(' · ');
	});

	// WHEN IT RUNS, as a sentence with the settings picked out.
	const sentence = $derived.by(() => {
		if (!job) return [];
		const out: { t: string; chip?: boolean; mono?: boolean }[] = [];
		const words = job.schedule_words ?? '';
		const [often, window] = words.split(', from ');
		if (job.trigger === 'schedule') {
			out.push({ t: 'Run ' }, { t: often.replace(/^./, (x) => x.toLowerCase()), chip: true });
			if (window) out.push({ t: ' from ' }, { t: window, chip: true });
		} else if (job.trigger === 'files') {
			out.push({ t: 'Run when new files land in ' }, { t: job.watch_path || 'your files', chip: true });
			if (job.settle_secs) out.push({ t: ', after ' }, { t: `waiting ${span(job.settle_secs)}`, chip: true }, { t: ' for more' });
		} else {
			out.push({ t: 'Run ' }, { t: 'only when you press Run', chip: true });
		}
		const args = ((job.setup.args as string[]) ?? []).join(' ');
		if (args) out.push({ t: ' with ' }, { t: args, chip: true, mono: true });
		const n = job.notify ?? {};
		out.push({ t: ', and ' }, { t: n.failed === false ? "don't tell me" : n.finished ? 'tell me when it finishes' : 'tell me if it fails', chip: true }, { t: '.' });
		return out;
	});

	// What it makes: the files recent runs wrote.
	const makes = $derived.by(() => {
		const seen = new Map<string, { path: string; size: number; at: string | null }>();
		for (const r of runs.slice(0, 10))
			for (const ch of (r.changes ?? []) as { path: string; kind: string; size: number }[])
				if (ch.kind !== 'deleted' && !seen.has(ch.path)) seen.set(ch.path, { path: ch.path, size: ch.size, at: r.ended_at });
		return [...seen.values()].slice(0, 5);
	});

	async function runNow() {
		try {
			const r = await post(`/jobs/${id}/run`);
			goto(`/scripts/${id}?tab=runs&run=${r.run_id}`);
			load();
		} catch (e) {
			toast(message(e), true);
		}
	}
	async function stop() {
		if (!last) return;
		try {
			await post(`/runs/${last.id}/stop`);
			load();
		} catch (e) {
			toast(message(e), true);
		}
	}
	async function setStatus(status: string) {
		try {
			job = await patch(`/jobs/${id}`, { status });
			toast(status === 'paused' ? 'Paused. It won’t run until you resume it.' : 'Resumed');
			loadScripts();
			load();
		} catch (e) {
			toast(message(e), true);
		}
	}
	async function remove() {
		if (!confirm(`Delete ${job?.name}? Its run history goes too. The script file stays in Files.`)) return;
		await del(`/jobs/${id}`);
		await loadScripts();
		goto('/scripts');
	}

	const tabHref = (t: string) => `/scripts/${id}${t === 'overview' ? '' : `?tab=${t}`}`;
	const runIndex = $derived(runs.findIndex((r) => r.id === runId));
	const runHref = (r: RunBrief | undefined) => (r ? `/scripts/${id}?tab=runs&run=${r.id}` : undefined);
	const runTook = $derived(run?.started_at ? ((run.ended_at ? new Date(run.ended_at).getTime() : Date.now()) - new Date(run.started_at).getTime()) / 1000 : null);
</script>

{#if job && st}
	<div class="wrap">
		<div class="hd">
			<div class="ht">
				<div class="tl"><h1 class="mono">{job.name}</h1><Status status={st.status} word={st.word} /></div>
				<p class="sub">{sub}</p>
			</div>
			<div class="ha">
				{#if !going}<Button icon="braces" href="/code?project={encodeURIComponent(folder)}">Open in Code</Button>{/if}
				{#if going}
					<Button icon="stop" onclick={stop}>Stop</Button>
				{:else if last && ['failed', 'timed_out'].includes(last.status)}
					<Button icon="refresh" onclick={runNow}>Run again</Button>
				{:else}
					<Button variant="primary" icon="play" onclick={runNow}>Run now</Button>
				{/if}
			</div>
		</div>

		<nav class="tabs" aria-label="Script">
			{#each [['overview', 'Overview'], ['runs', 'Runs'], ['code', 'Code'], ['settings', 'Settings']] as [t, l] (t)}
				<a href={tabHref(t)} class:on={tab === t} aria-current={tab === t ? 'page' : undefined}>{l}</a>
			{/each}
		</nav>

		{#if tab === 'overview'}
			<div class="ov">
				<div class="col">
					<div class="box">
						<span class="eyebrow">When it runs</span>
						<p class="sent" data-testid="when-it-runs">{#each sentence as s, i (i)}{#if s.chip}<span class="schip" class:mono={s.mono}>{s.t}</span>{:else}{s.t}{/if}{/each}</p>
					</div>
					<div class="lh">
						<span class="h2">Last run</span>
						{#if last}<span class="lm"><span class="mono">{clock(last.ended_at ?? last.started_at ?? last.queued_at)}</span>{#if last.started_at && last.ended_at} · took <span class="mono">{span((new Date(last.ended_at).getTime() - new Date(last.started_at).getTime()) / 1000)}</span>{/if} · <Status status={last.status} word={last.status === 'succeeded' ? 'Finished' : undefined} /></span>{/if}
					</div>
					{#if last}
						<a class="lastlink" href={runHref(last)} aria-label="Open the last run">{#key last.id}<Output runId={last.id} live={isActive(last.status)} tail={8} height={180} />{/key}</a>
					{:else}
						<div class="empty">It hasn't run yet. <button class="link" onclick={runNow}>Run it now</button></div>
					{/if}
					{#if runs.length}
						<div class="box">
							<RunsChart runs={runs.slice(0, 14)} onpick={(rid) => goto(`/scripts/${id}?tab=runs&run=${rid}`)} />
						</div>
					{/if}
				</div>
				<aside class="col side">
					<div class="box">
						<span class="h3">What it makes</span>
						{#each makes as f (f.path)}
							<a class="kvr" href="/files?path={encodeURIComponent(f.path.split('/').slice(0, -1).join('/'))}"><Icon name="file" size={14} /><span class="mono grow">{f.path}</span><span class="mid small">{bytes(f.size)} · {clock(f.at)}</span></a>
						{:else}
							<p class="mid small">No files yet. Files a run writes show here.</p>
						{/each}
					</div>
					<div class="box">
						<span class="h3">What it needs</span>
						<div class="kv"><span>Packages</span><span class="r">{#if packages?.names.length}<span class="mono">{packages.names.join(', ')}</span><span class="mid small">from {packages.from}</span>{:else}<span class="mid">None</span>{/if}</span></div>
						<div class="kv"><span>Secrets</span><span class="r">{#if job.secret_names.length}{#each job.secret_names as s (s)}<span class="mono"><Icon name="key" size={12} /> {s}</span>{/each}{:else}<span class="mid">None</span>{/if}</span></div>
						<div class="kv"><span>Runs on</span><span class="r">{c?.name ?? 'Your computer'}{c ? ` · ${c.memory_gb} GB memory` : ''}</span></div>
					</div>
					<p class="mid small">Runs on {c?.name ?? 'your computer'}. It wakes up for each run, then goes back to sleep.</p>
				</aside>
			</div>
		{:else if tab === 'runs'}
			{#if run && run.id === runId}
				<div class="nav">
					<a class="arrow" class:off={runIndex >= runs.length - 1} href={runHref(runs[runIndex + 1])} aria-label="Earlier run"><Icon name="chevron-left" size={14} /></a>
					<span class="nt">{dayAndTime(run.started_at ?? run.queued_at).replace(/^./, (x) => x.toUpperCase())}</span>
					<a class="arrow" class:off={runIndex <= 0} href={runHref(runs[runIndex - 1])} aria-label="Later run"><Icon name="chevron-right" size={14} /></a>
					{#if !isActive(run.status)}<span class="mid">{runTook != null ? `took ${span(runTook)}` : ''}{run.exit_code != null ? ` · exit code ${run.exit_code}` : ''}</span>{/if}
					<Status status={run.status} word={run.status === 'succeeded' ? 'Finished' : run.status === 'running' ? 'Running' : undefined} />
					{#if run.trigger !== 'manual'}<span class="mid small">{run.trigger === 'schedule' ? 'on schedule' : run.trigger === 'files' ? 'when files changed' : run.trigger === 'retry' ? 'retry' : run.trigger}</span>{/if}
				</div>
				<RunView {run} {job} {runs} onchange={() => (load(), loadRun())} />
			{:else if !runs.length}
				<div class="empty">No runs yet. <button class="link" onclick={runNow}>Run it now</button></div>
			{/if}
		{:else if tab === 'code'}
			<div class="codebar"><span class="mono mid">Files › {job.setup.path.split('/').join(' › ')}</span><Button size="sm" icon="braces" href="/code?project={encodeURIComponent(folder)}">Edit in Code</Button></div>
			{#if source}
				<div class="code mono" data-testid="source">{#each source as l, i (i)}<div class="cl"><span class="n">{i + 1}</span><span class="t">{l}</span></div>{/each}</div>
			{:else}
				<div class="empty">Opening {file}…</div>
			{/if}
		{:else if tab === 'settings'}
			{#key job.id}
				<div class="settings">
					<ScriptAdd {job} onsaved={(j) => ((job = { ...job!, ...j }), loadScripts(), toast('Saved'))} />
					<div class="danger">
						{#if job.status === 'paused'}
							<Button icon="play" onclick={() => setStatus('active')}>Resume</Button>
						{:else}
							<Button icon="pause" onclick={() => setStatus('paused')}>Pause</Button>
						{/if}
						<Button variant="danger" icon="trash" onclick={remove}>Delete script</Button>
						<span class="mid small">Deleting removes its runs. The file stays in Files.</span>
					</div>
				</div>
			{/key}
		{/if}
	</div>
{/if}

<style>
	.wrap {
		padding: 22px 28px 28px;
		display: flex;
		flex-direction: column;
		gap: 16px;
		min-height: 100%;
	}
	.settings {
		margin: -24px -28px 0;
	}
	.settings :global(.add) {
		padding-top: 24px;
	}
	.hd {
		display: flex;
		justify-content: space-between;
		gap: 16px;
		align-items: flex-start;
	}
	.tl {
		display: flex;
		align-items: center;
		gap: 12px;
	}
	h1 {
		font-size: 24px;
		font-weight: 500;
		margin: 0;
	}
	.sub {
		margin: 4px 0 0;
		font-size: 13.5px;
		color: var(--mid);
	}
	.ha {
		display: flex;
		gap: 8px;
		flex-shrink: 0;
	}
	.tabs {
		display: flex;
		gap: 22px;
		border-bottom: 1px solid var(--line);
	}
	.tabs a {
		padding: 8px 0 10px;
		color: var(--mid);
		border-bottom: 2px solid transparent;
		margin-bottom: -1px;
		font-size: 15px;
	}
	.tabs a:hover {
		text-decoration: none;
		color: var(--ink);
	}
	.tabs a.on {
		color: var(--ink);
		border-bottom-color: var(--ink);
	}
	.ov {
		display: grid;
		grid-template-columns: minmax(0, 1fr) 320px;
		gap: 20px;
		align-items: start;
	}
	.col {
		display: flex;
		flex-direction: column;
		gap: 14px;
		min-width: 0;
	}
	.box {
		border: 1px solid var(--line);
		border-radius: 12px;
		background: var(--pane);
		padding: 16px 18px;
		display: flex;
		flex-direction: column;
		gap: 10px;
	}
	.eyebrow {
		font: 11px 'JetBrains Mono', ui-monospace, monospace;
		letter-spacing: 0.08em;
		text-transform: uppercase;
		color: var(--mid);
	}
	.sent {
		margin: 0;
		font-size: 16px;
		line-height: 2;
	}
	.schip {
		display: inline-block;
		padding: 0 8px;
		line-height: 1.7;
		border-radius: 6px;
		background: var(--live-soft);
		color: var(--accent-ink);
	}
	.schip.mono {
		font-size: 13px;
	}
	.lh {
		display: flex;
		justify-content: space-between;
		align-items: center;
	}
	.h2 {
		font-size: 16px;
		font-weight: 600;
	}
	.h3 {
		font-size: 15px;
		font-weight: 600;
	}
	.lm {
		display: flex;
		align-items: center;
		gap: 6px;
		font-size: 13px;
		color: var(--mid);
	}
	.lastlink {
		color: inherit;
	}
	.lastlink:hover {
		text-decoration: none;
	}
	.kvr {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 8px 0;
		border-top: 1px solid var(--line);
		color: var(--ink);
		font-size: 13px;
	}
	.kvr:hover {
		text-decoration: none;
	}
	.grow {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.kv {
		display: flex;
		justify-content: space-between;
		gap: 12px;
		padding: 10px 0;
		border-top: 1px solid var(--line);
		font-size: 13.5px;
		color: var(--mid);
	}
	.kv .r {
		display: flex;
		flex-direction: column;
		align-items: flex-end;
		text-align: right;
		color: var(--ink);
	}
	.small {
		font-size: 12.5px;
	}
	.empty {
		padding: 24px;
		border: 1px dashed var(--line-strong);
		border-radius: 12px;
		color: var(--mid);
		text-align: center;
	}
	.link {
		background: none;
		border: 0;
		padding: 0;
		color: var(--accent-ink);
		text-decoration: underline;
		cursor: pointer;
		font: inherit;
	}
	.nav {
		display: flex;
		align-items: center;
		gap: 10px;
		font-size: 14px;
	}
	.arrow {
		width: 30px;
		height: 30px;
		display: flex;
		align-items: center;
		justify-content: center;
		border: 1px solid var(--line);
		border-radius: 8px;
		color: var(--ink);
	}
	.arrow.off {
		pointer-events: none;
		opacity: 0.35;
	}
	.nt {
		font-weight: 600;
		font-size: 15px;
	}
	.codebar {
		display: flex;
		justify-content: space-between;
		align-items: center;
	}
	.code {
		border: 1px solid var(--line);
		border-radius: 12px;
		background: var(--pane);
		padding: 12px 0;
		font-size: 13px;
		overflow: auto;
	}
	.cl {
		display: flex;
		gap: 14px;
		padding: 0 16px;
		line-height: 1.6;
		white-space: pre;
	}
	.n {
		width: 28px;
		text-align: right;
		color: var(--low);
		flex-shrink: 0;
		user-select: none;
	}
	.danger {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 0 28px 24px;
	}
	@media (max-width: 1000px) {
		.ov {
			grid-template-columns: 1fr;
		}
	}
</style>
