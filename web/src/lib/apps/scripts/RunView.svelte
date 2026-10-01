<script lang="ts">
	import { get, patch, post, message } from '$lib/api';
	import { refreshMe, session } from '$lib/session.svelte';
	import { bytes, dayAndTime, span, timeOfDay } from '$lib/format';
	import type { Job, RunBrief } from '$lib/types';
	import Button from '$lib/ui/Button.svelte';
	import Icon from '$lib/ui/Icon.svelte';
	import Sparkline from '$lib/ui/Sparkline.svelte';
	import Status from '$lib/ui/Status.svelte';
	import { toast } from '$lib/ui/toast.svelte';
	import Output from './Output.svelte';
	import { isActive } from './store.svelte';

	// One run of a script: in progress (how far, CPU and memory, what to do when it ends),
	// failed (why, the output, and where in the script it stopped), or finished (what it
	// found, the files it made, and how it compares).
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	let { run, job, runs, onchange }: { run: any; job: Job; runs: RunBrief[]; onchange: () => void } = $props();

	const c = $derived(session.me?.computers.find((x) => x.id === run.computer_id));
	const active = $derived(isActive(run.status));
	const failed = $derived(['failed', 'timed_out'].includes(run.status));
	const took = $derived(run.started_at ? ((run.ended_at ? new Date(run.ended_at).getTime() : Date.now()) - new Date(run.started_at).getTime()) / 1000 : 0);

	// --- In progress ---------------------------------------------------------------------
	const p = $derived(run.progress ?? {});
	const frac = $derived(p.total ? Math.min(1, (p.done ?? 0) / p.total) : null);
	const eta = $derived(frac && frac > 0.02 && took > 5 ? (took / frac) * (1 - frac) : null);
	const cpuHist = $derived<number[]>((p.history ?? []).map((h: [number, number]) => h[0]));
	const memHist = $derived<number[]>((p.history ?? []).map((h: [number, number]) => h[1]));
	const cores = $derived(c?.cpu ?? 1);
	let tellMe = $state(false);
	$effect(() => {
		tellMe = !!run.tell_me;
	});
	async function setTellMe(on: boolean) {
		try {
			await post(`/runs/${run.id}/tell-me`, { on });
			toast(on ? "We'll tell you when it finishes" : 'Okay');
		} catch (e) {
			toast(message(e), true);
		}
	}
	async function setKeepAwake(on: boolean) {
		try {
			await patch(`/computers/${run.computer_id}`, { keep_awake: on });
			await refreshMe();
		} catch (e) {
			toast(message(e), true);
			await refreshMe();
		}
	}

	// --- Failed: where it stopped ------------------------------------------------------------
	let source = $state<string[] | null>(null);
	const scriptName = $derived(job.setup.path.split('/').pop() ?? '');
	const stopLine = $derived.by(() => {
		const text: string = (run.output ?? []).map((l: { text: string }) => l.text).join('\n') + '\n' + (run.output_tail ?? '');
		const esc = scriptName.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
		const pats = [new RegExp(`${esc}", line (\\d+)`, 'g'), new RegExp(`${esc}: line (\\d+)`, 'g'), new RegExp(`${esc}:(\\d+)`, 'g')];
		let last: number | null = null;
		for (const re of pats) for (const m of text.matchAll(re)) last = parseInt(m[1], 10);
		return last;
	});
	$effect(() => {
		if (!failed) return;
		get(`/computers/${run.computer_id}/files/preview?path=${encodeURIComponent(job.setup.path)}`)
			.then((r) => (source = r.kind === 'text' ? r.text.split('\n') : null))
			.catch(() => (source = null));
	});
	const excerpt = $derived.by(() => {
		if (!source) return [];
		const at = stopLine ?? 0;
		const from = Math.max(1, at ? at - 7 : 1);
		return source.slice(from - 1, from + 11).map((t, i) => ({ n: from + i, t }));
	});

	// --- Finished: files, where it went, compare ---------------------------------------------
	const made = $derived<{ path: string; kind: string; size: number }[]>((run.changes ?? []).filter((x: { kind: string }) => x.kind !== 'deleted'));
	let pick = $state<string | null>(null);
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	let preview = $state<any>(null);
	$effect(() => {
		pick = made[0]?.path ?? null;
	});
	$effect(() => {
		const path = pick;
		preview = null;
		if (!path || active) return;
		get(`/computers/${run.computer_id}/files/preview?path=${encodeURIComponent(path)}`)
			.then((r) => (preview = r))
			.catch(() => (preview = null));
	});
	const notified = $derived.by(() => {
		const n = job.notify ?? {};
		if (failed) return n.failed === false ? 'No. You turned off failure alerts.' : 'Yes, it told you it failed.';
		if (run.tell_me || n.finished) return 'Yes, when it finished.';
		return 'No. You asked to hear only about failures.';
	});
	const compare = $derived.by(() => {
		const label = run.summary?.values?.[0]?.label;
		if (!label) return null;
		const num = (v?: string) => (v == null ? NaN : parseFloat(String(v).replace(/[^0-9.-]/g, '')));
		const past = runs
			.filter((r) => r.id !== run.id && r.status === 'succeeded' && r.trigger !== 'test' && r.summary?.values?.some((v) => v.label === label))
			.slice(0, 14);
		if (!past.length) return null;
		const vals = past.map((r) => num(r.summary!.values.find((v) => v.label === label)!.value)).filter((v) => !isNaN(v));
		const prev = past[0];
		const avg = vals.length ? vals.reduce((a, b) => a + b, 0) / vals.length : NaN;
		return {
			label,
			prev: prev.summary!.values.find((v) => v.label === label)!.value,
			at: timeOfDay(prev.ended_at),
			avg: isNaN(avg) ? null : Number.isInteger(avg) ? avg : avg.toFixed(1),
			n: vals.length
		};
	});

	async function retry() {
		try {
			await post(`/runs/${run.id}/retry`);
			onchange();
		} catch (e) {
			toast(message(e), true);
		}
	}
	let fullLog = $state(false);
</script>

{#if active}
	<div class="progress card">
		<div class="ph">
			{#if p.done != null}
				<span class="step"><strong>Step</strong> <span class="mono">{p.done.toLocaleString('en-US')}</span>{#if p.total}&nbsp;<strong>of</strong> <span class="mono">{p.total.toLocaleString('en-US')}</span>{/if}</span>
			{:else}
				<span class="step"><strong>{run.status === 'running' ? 'Running' : (run.status_note ?? 'Getting ready')}</strong></span>
			{/if}
			<span class="eta">{#if eta}About <span class="mono">{span(eta)}</span> left · done around <span class="mono">{timeOfDay(new Date(Date.now() + eta * 1000).toISOString())}</span>{:else}Running for <span class="mono">{span(took)}</span>{/if}</span>
		</div>
		<span class="pbar"><span style="width: {frac != null ? Math.round(frac * 100) : 100}%" class:indet={frac == null}></span></span>
		<span class="note">Progress is read from lines like <span class="mono">step 6,200</span> or <span class="mono">6200/10000</span> in your script's output.</span>
	</div>
	<div class="three">
		<div class="card mini">
			<div class="mh"><span class="label">CPU</span><span class="mono big">{Math.round(p.cpu_percent ?? 0)}%</span></div>
			<Sparkline values={cpuHist} max={100 * cores} tone="working" />
			<span class="note">{Math.min(cores, Math.max(0, Math.round((p.cpu_percent ?? 0) / 100)))} of {cores} core{cores === 1 ? '' : 's'} busy</span>
		</div>
		<div class="card mini">
			<div class="mh"><span class="label">Memory</span><span class="mono big">{((p.memory_mb ?? 0) / 1024).toFixed(1)} GB</span></div>
			<Sparkline values={memHist} max={(c?.memory_gb ?? 1) * 1024} tone="needs" />
			<span class="note">of {c?.memory_gb ?? '?'} GB</span>
		</div>
		<div class="card mini">
			<span class="label">While it runs</span>
			<label class="tick"><input type="checkbox" bind:checked={tellMe} onchange={() => setTellMe(tellMe)} /><span>Tell me when it finishes</span></label>
			<label class="tick"><input type="checkbox" checked={!!c?.keep_awake} onchange={(e) => setKeepAwake(e.currentTarget.checked)} /><span>Keep the computer awake<span class="sub">Otherwise it sleeps once the run ends</span></span></label>
		</div>
	</div>
	<div class="oh"><span class="h2">Output</span><span class="live"><span class="dot"></span>Live</span></div>
	<Output runId={run.id} initial={run.output} height={380} />
	<p class="note foot">Keeps running if you close this tab. We'll let you know when it's done.</p>
{:else if failed}
	<div class="cols">
		<div class="col">
			<div class="errbox" data-testid="why">
				<div class="et"><Icon name="warning" size={18} /><span>{run.error_plain ?? 'It stopped with an error.'}</span></div>
				{#if run.error_fix}<p>{run.error_fix}</p>{/if}
				<div class="ea">
					<Button variant="primary" icon="refresh" onclick={retry}>Run again</Button>
					<Button variant="quiet" icon="sparkle" onclick={() => (session.assistantOpen = true)}>Ask the assistant</Button>
				</div>
			</div>
			<div class="oh"><span class="h2">Output</span><span class="mono note">{timeOfDay(run.started_at)} · {span(took)} · exit code {run.exit_code ?? '–'}</span></div>
			<Output runId={run.id} initial={run.output} live={false} height={260} />
			{#if excerpt.length}
				<span class="h2">Where it stopped</span>
				<div class="code mono">
					{#each excerpt as l (l.n)}<div class="cl" class:hit={l.n === stopLine}><span class="n">{l.n}</span><span class="t">{l.t}</span></div>{/each}
				</div>
			{/if}
		</div>
		<aside class="side">
			<div class="card">
				<span class="h3">Recent runs</span>
				{#each runs.filter((r) => r.ended_at && r.trigger !== 'test').slice(0, 6) as r (r.id)}
					<div class="rr"><span class="mono">{dayAndTime(r.ended_at).replace(/^./, (x) => x.toUpperCase())}</span><span class="grow"></span><span class="mono note">{r.started_at ? span((new Date(r.ended_at!).getTime() - new Date(r.started_at).getTime()) / 1000) : ''}</span><Status status={r.status} word={r.status === 'succeeded' ? 'Finished' : undefined} /></div>
				{/each}
			</div>
		</aside>
	</div>
{:else}
	<div class="cols">
		<div class="col">
			{#if run.summary?.values?.length}
				<div class="card result">
					<div class="rh"><span class="label">Result</span><span class="note">Written by the script</span></div>
					<div class="vals">
						{#each run.summary.values.slice(0, 4) as v (v.label)}<div class="v"><span class="mono vv">{v.value}</span><span class="vl">{v.label}</span></div>{/each}
					</div>
				</div>
			{:else}
				<div class="card result"><span class="label">Result</span><p class="hl" data-testid="headline">{run.headline}</p></div>
			{/if}
			{#if made.length}
				<div class="chips">
					{#each made.slice(0, 4) as f (f.path)}
						<button class="chip" class:on={pick === f.path} onclick={() => (pick = f.path)}><Icon name="file" size={13} /><span class="mono">{f.path.split('/').pop()}</span><span class="mono note">{f.kind === 'created' ? 'new' : 'changed'} · {bytes(f.size)}</span></button>
					{/each}
					<span class="grow"></span>
					{#if pick}
						<Button variant="quiet" icon="data" disabled title="The Data app is coming soon">Open in Data</Button>
						<Button variant="quiet" icon="download-line" href={`/api/computers/${run.computer_id}/files/download?path=${encodeURIComponent(pick)}`}>Download</Button>
					{/if}
				</div>
				{#if preview?.kind === 'table'}
					<div class="tbl">
						<table><thead><tr>{#each preview.columns as col, i (i)}<th>{col}</th>{/each}</tr></thead><tbody>{#each preview.rows.slice(0, 8) as r, i (i)}<tr>{#each r as cell, j (j)}<td>{cell}</td>{/each}</tr>{/each}</tbody></table>
						<div class="tf"><span>Showing {Math.min(8, preview.rows.length)} of {preview.total_rows.toLocaleString('en-US')} rows</span><a class="mono" href="/files?path={encodeURIComponent(pick!.split('/').slice(0, -1).join('/'))}&open={encodeURIComponent(pick!)}">{pick}</a></div>
					</div>
				{:else if preview?.kind === 'text'}
					<pre class="txt mono">{preview.text.slice(0, 2000)}</pre>
				{:else if preview?.kind === 'image'}
					<img class="img" src={preview.data_url} alt={pick} />
				{/if}
			{/if}
		</div>
		<aside class="side">
			<div class="card">
				<span class="h3">Where it went</span>
				<div class="kv"><span>Saved to</span><span>{#if made[0]}<a href="/files?path={encodeURIComponent(made[0].path.split('/').slice(0, -1).join('/'))}" class="mono">Files › {made[0].path.split('/').join(' › ')}</a>{:else}Nothing new in Files{/if}</span></div>
				<div class="kv"><span>Shown in Home</span><span>“{#if run.summary?.values?.length}<span data-testid="headline">{run.headline}</span>{:else}{run.headline}{/if}”</span></div>
				<div class="kv"><span>Notified you</span><span>{notified}</span></div>
			</div>
			<div class="card">
				<div class="rh"><span class="h3">Output</span><button class="lnk" onclick={() => (fullLog = !fullLog)}>{fullLog ? 'Last lines' : 'Full log'}</button></div>
				<Output runId={run.id} initial={run.output} live={false} tail={fullLog ? 0 : 3} height={fullLog ? 420 : 120} />
			</div>
			{#if compare}
				<div class="card">
					<span class="h3">Compare</span>
					<p class="note">Last run at {compare.at}: {compare.label.toLowerCase()} <strong>{compare.prev}</strong>.{#if compare.avg != null} Average over {compare.n} run{compare.n === 1 ? '' : 's'}: <strong>{compare.avg}</strong>.{/if}</p>
				</div>
			{/if}
		</aside>
	</div>
{/if}

<style>
	.card {
		border: 1px solid var(--line);
		border-radius: 12px;
		padding: 16px 18px;
		display: flex;
		flex-direction: column;
		gap: 10px;
		background: var(--surface);
	}
	.label {
		font: 500 11px/16px 'JetBrains Mono', ui-monospace, monospace;
		letter-spacing: 0.08em;
		text-transform: uppercase;
		color: var(--low);
	}
	.note {
		font-size: 13px;
		color: var(--mid);
	}
	.h2 {
		font-size: 16px;
		font-weight: 600;
	}
	.h3 {
		font-size: 15px;
		font-weight: 600;
	}
	.grow {
		flex: 1;
	}
	.ph {
		display: flex;
		justify-content: space-between;
		gap: 12px;
		font-size: 16px;
	}
	.step .mono {
		font-size: 16px;
	}
	.eta {
		font-size: 13px;
		color: var(--mid);
	}
	.eta .mono {
		color: var(--ink);
	}
	.pbar {
		height: 8px;
		border-radius: 4px;
		background: var(--line);
		overflow: hidden;
	}
	.pbar span {
		display: block;
		height: 8px;
		border-radius: 4px;
		background: var(--working);
	}
	.pbar .indet {
		opacity: 0.4;
		animation: pulse 1.4s ease-in-out infinite;
	}
	@keyframes pulse {
		50% {
			opacity: 0.15;
		}
	}
	.three {
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		gap: 12px;
	}
	.mini {
		gap: 8px;
	}
	.mh {
		display: flex;
		justify-content: space-between;
		align-items: baseline;
	}
	.big {
		font-size: 15px;
	}
	.tick {
		display: flex;
		gap: 10px;
		align-items: flex-start;
		font-size: 14px;
		cursor: pointer;
	}
	.tick input {
		margin-top: 3px;
	}
	.tick .sub {
		display: block;
		font-size: 12px;
		color: var(--mid);
	}
	.oh,
	.rh {
		display: flex;
		justify-content: space-between;
		align-items: baseline;
	}
	.live {
		display: flex;
		align-items: center;
		gap: 6px;
		font-size: 13px;
		color: var(--mid);
	}
	.dot {
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background: var(--working);
	}
	.foot {
		margin-top: auto;
	}
	.cols {
		display: grid;
		grid-template-columns: minmax(0, 1fr) 300px;
		gap: 16px;
		align-items: start;
	}
	.col,
	.side {
		display: flex;
		flex-direction: column;
		gap: 14px;
		min-width: 0;
	}
	.errbox {
		border: 1px solid var(--failed);
		border-radius: 12px;
		background: var(--failed-soft);
		padding: 18px;
		display: flex;
		flex-direction: column;
		gap: 10px;
	}
	.et {
		display: flex;
		gap: 10px;
		align-items: center;
		font-size: 16px;
		font-weight: 600;
		color: var(--failed);
	}
	.et span {
		color: var(--ink);
	}
	.errbox p {
		font-size: 14px;
		line-height: 21px;
		color: var(--mid);
	}
	.ea {
		display: flex;
		gap: 8px;
		flex-wrap: wrap;
	}
	.code {
		border: 1px solid var(--line);
		border-radius: 10px;
		background: var(--pane);
		padding: 10px 0;
		font-size: 12.5px;
		line-height: 21px;
		overflow: auto;
	}
	.cl {
		display: flex;
		gap: 12px;
		padding: 0 14px;
		border-left: 2px solid transparent;
	}
	.cl.hit {
		background: var(--failed-soft);
		border-left-color: var(--failed);
	}
	.cl .n {
		width: 28px;
		text-align: right;
		color: var(--low);
		flex-shrink: 0;
	}
	.cl .t {
		white-space: pre;
	}
	.rr {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 8px 0;
		border-top: 1px solid var(--line);
		font-size: 13px;
	}
	.result .vals {
		display: grid;
		grid-template-columns: repeat(4, minmax(0, 1fr));
		gap: 12px;
	}
	.v {
		display: flex;
		flex-direction: column;
	}
	.vv {
		font-size: 26px;
		font-weight: 500;
	}
	.vl {
		font-size: 13px;
		color: var(--mid);
	}
	.hl {
		font-size: 16px;
	}
	.chips {
		display: flex;
		align-items: center;
		gap: 8px;
		flex-wrap: wrap;
	}
	.chip {
		display: flex;
		align-items: center;
		gap: 8px;
		height: 34px;
		padding: 0 12px;
		border-radius: 8px;
		border: 1px solid transparent;
		background: none;
		color: var(--ink);
		font: 13px Inter, system-ui, sans-serif;
		cursor: pointer;
	}
	.chip.on {
		border-color: var(--line);
		background: var(--pane);
	}
	.chip .mono {
		font-size: 13px;
	}
	.chip .note {
		font-size: 11px;
	}
	.tbl {
		border: 1px solid var(--line);
		border-radius: 10px;
		overflow: auto;
	}
	.tbl table {
		width: 100%;
		border-collapse: collapse;
		font-size: 14px;
	}
	.tbl th {
		font: 400 11px 'JetBrains Mono', ui-monospace, monospace;
		color: var(--low);
		text-align: left;
		padding: 8px 14px;
		border-bottom: 1px solid var(--line);
	}
	.tbl td {
		padding: 7px 14px;
		border-bottom: 1px solid var(--line);
	}
	.tf {
		display: flex;
		justify-content: space-between;
		gap: 12px;
		padding: 10px 14px;
		font-size: 13px;
		color: var(--mid);
	}
	.tf .mono {
		font-size: 12px;
		color: var(--mid);
	}
	.txt {
		border: 1px solid var(--line);
		border-radius: 10px;
		background: var(--pane);
		padding: 12px 14px;
		font-size: 12px;
		max-height: 300px;
		overflow: auto;
		white-space: pre-wrap;
		margin: 0;
	}
	.img {
		max-width: 100%;
		border-radius: 8px;
	}
	.kv {
		display: flex;
		flex-direction: column;
		gap: 2px;
		padding: 10px 0;
		border-top: 1px solid var(--line);
		font-size: 13px;
	}
	.kv > span:first-child {
		color: var(--mid);
	}
	.kv .mono {
		font-size: 13px;
	}
	.lnk {
		background: none;
		border: 0;
		padding: 0;
		font: 13px Inter, system-ui, sans-serif;
		color: var(--accent-ink);
		text-decoration: underline;
		text-underline-offset: 3px;
		cursor: pointer;
	}
	@media (max-width: 1100px) {
		.cols {
			grid-template-columns: minmax(0, 1fr);
		}
		.three {
			grid-template-columns: minmax(0, 1fr);
		}
	}
</style>
