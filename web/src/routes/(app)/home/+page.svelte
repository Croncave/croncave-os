<script lang="ts">
	import { goto } from '$app/navigation';
	import { get, post, message } from '$lib/api';
	import { onLive, throttle } from '$lib/live';
	import { session, setComputer } from '$lib/session.svelte';
	import { ago, appName, greeting, when } from '$lib/format';
	import Status from '$lib/ui/Status.svelte';
	import Button from '$lib/ui/Button.svelte';
	import { toast } from '$lib/ui/toast.svelte';

	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	let home = $state<any>(null);
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	let jobs = $state<any[]>([]);
	let q = $state('');

	async function load() {
		home = await get('/home');
		if (session.computerId) jobs = (await get(`/computers/${session.computerId}/jobs`)).jobs;
	}
	$effect(() => {
		void session.computerId;
		load();
	});
	$effect(() => onLive(throttle(load, 400)));

	const apps = [
		{ name: 'Files', href: '/files' },
		{ name: 'Scripts', href: '/scripts' },
		{ name: 'Watcher', href: '/watcher' },
		{ name: 'Code', href: '/code' },
		{ name: 'Plans and billing', href: '/plans' },
		{ name: 'Settings', href: '/settings' }
	];
	const matches = $derived(
		q.trim()
			? [
					...apps.filter((a) => a.name.toLowerCase().includes(q.toLowerCase())).map((a) => ({ label: a.name, sub: 'App', href: a.href })),
					...jobs.filter((j) => j.name.toLowerCase().includes(q.toLowerCase())).map((j) => ({ label: j.name, sub: appName(j.app), href: `/${j.app}/${j.id}` }))
				]
			: []
	);

	async function command(e: SubmitEvent) {
		e.preventDefault();
		if (matches[0]) return goto(matches[0].href);
		if (session.me?.account.ai_enabled) {
			session.assistantOpen = true;
			return;
		}
		toast('Nothing matches. Turn on the assistant in Settings to ask in your own words.');
	}

	async function approve(a: { run_id: string; request_id: string }, ok: boolean) {
		try {
			await post(`/runs/${a.run_id}/approvals/${a.request_id}`, { approved: ok });
			load();
		} catch (e) {
			toast(message(e), true);
		}
	}
	const goRun = (run: string, computer: string) => (setComputer(computer), goto(`/runs/${run}`));
	const needs = $derived(home ? home.needs_you.approvals.length + home.needs_you.reviews.length + home.needs_you.failing.length : 0);
</script>

<div class="page">
	<div class="stack" style="gap: 6px">
		<h1>{greeting(session.me?.user.name ?? '')}</h1>
		{#if home}
			<p class="mid" data-testid="since">
				Since you left {home.since ? ago(home.since) : ''}:
				{#if !home.happened.length}nothing new.{:else}
					{home.counts.success ?? 0} result{(home.counts.success ?? 0) === 1 ? '' : 's'}{#if home.counts.failed}, {home.counts.failed} failed{/if}{#if needs}, {needs} need{needs === 1 ? 's' : ''} you{/if}.
				{/if}
			</p>
		{/if}
	</div>
	<form class="cmd" onsubmit={command}>
		<input bind:value={q} placeholder={session.me?.account.ai_enabled ? 'Search apps and jobs, or ask the assistant' : 'Search apps and jobs'} aria-label="Command bar" />
		{#if matches.length}
			<div class="results card">
				{#each matches.slice(0, 8) as m (m.href)}<a href={m.href} class="row"><span>{m.label}</span><span class="spacer"></span><span class="low">{m.sub}</span></a>{/each}
			</div>
		{/if}
	</form>

	{#if home}
		{#if needs}
			<section class="card stack">
				<h2>Needs you</h2>
				<div class="list">
					{#each home.needs_you.approvals as a (a.request_id)}
						<div class="row" data-testid="approval">
							<Status status="needs_you" /><span>The agent working on <strong>{a.job}</strong> wants to run <code>{a.command}</code></span><span class="spacer"></span>
							<Button size="sm" variant="primary" onclick={() => approve(a, true)}>Approve</Button><Button size="sm" onclick={() => approve(a, false)}>Deny</Button>
						</div>
					{/each}
					{#each home.needs_you.reviews as r (r.run_id)}
						<div class="row"><Status status="needs_you" /><span>{r.title}</span><span class="spacer"></span><Button size="sm" onclick={() => (setComputer(r.computer_id), goto(`/code?review=${r.run_id}`))}>Review</Button></div>
					{/each}
					{#each home.needs_you.failing as f (f.job_id)}
						<div class="row"><Status status="failed" /><span><strong>{f.name}</strong>: {f.why}</span><span class="spacer"></span><Button size="sm" onclick={() => goRun(f.run_id, f.computer_id)}>See why</Button></div>
					{/each}
				</div>
			</section>
		{/if}
		<div class="grid2">
			<section class="card stack">
				<h2>What happened</h2>
				<div class="list">
					{#each home.happened.slice(0, 8) as e (e.id)}
						<a class="row ev" href={e.run_id ? `/runs/${e.run_id}` : '#'}><span>{e.title}</span><span class="spacer"></span><span class="low mono">{ago(e.created_at)}</span></a>
					{:else}
						<div class="low">Nothing yet. Set up a watch or a script and come back later.</div>
					{/each}
				</div>
			</section>
			<section class="card stack">
				<h2>Your computers</h2>
				<div class="list">
					{#each session.me?.computers ?? [] as c (c.id)}
						<div class="row"><button class="link" onclick={() => setComputer(c.id)}>{c.name}</button><span class="low">{c.size}</span><span class="spacer"></span><Status status={c.status} word={c.status_word} /></div>
					{/each}
				</div>
				<h3>Next up</h3>
				<div class="list">
					{#each home.upcoming as u (u.job_id)}
						<div class="row"><span>{u.name}</span><span class="low">{appName(u.app)}</span><span class="spacer"></span><span class="mono low">{when(u.at)}</span></div>
					{:else}
						<div class="low">Nothing scheduled.</div>
					{/each}
				</div>
			</section>
		</div>
		<section class="card stack">
			<h2>Latest results</h2>
			<div class="list">
				{#each home.results as r (r.run_id)}
					<button class="row res" onclick={() => goRun(r.run_id, r.computer_id)}><Status status={r.status} /><span>{r.job}</span><span class="mid">{r.headline ?? ''}</span><span class="spacer"></span><span class="low mono">{ago(r.at)}</span></button>
				{:else}
					<div class="low">No results yet.</div>
				{/each}
			</div>
		</section>
	{/if}
</div>

<style>
	.cmd {
		position: relative;
	}
	.cmd input {
		padding: 12px 14px;
		font-size: 15px;
	}
	.results {
		position: absolute;
		left: 0;
		right: 0;
		top: 52px;
		z-index: 10;
		display: grid;
		padding: 6px;
	}
	.results a {
		padding: 8px;
		border-radius: 6px;
		color: var(--ink);
	}
	.results a:hover {
		background: var(--pane);
		text-decoration: none;
	}
	.ev {
		color: var(--ink);
	}
	.res,
	.link {
		background: none;
		border: 0;
		color: inherit;
		font: inherit;
		text-align: left;
		cursor: pointer;
		width: 100%;
	}
	.link {
		width: auto;
		color: var(--accent-ink);
		padding: 0;
	}
</style>
