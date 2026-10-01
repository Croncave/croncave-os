<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { get, post, message } from '$lib/api';
	import { refreshMe, session, setComputer } from '$lib/session.svelte';
	import AppWindow from '$lib/shell/AppWindow.svelte';
	import Button from '$lib/ui/Button.svelte';
	import Icon from '$lib/ui/Icon.svelte';
	import { cap } from '$lib/plans';

	// A new computer, step by step, with what it is and costs in plain words beside it.
	// Sizes, storage and prices come from the catalog.
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	let catalog = $state<any>(null);
	let name = $state(session.me?.computers.length ? '' : 'My computer');
	let size = $state('small');
	let disk = $state(0);
	let apps = $state<Record<string, boolean>>({ watcher: true, scripts: true, code: false });
	let sleepAfter = $state(30);
	let wakeForSchedule = $state(true);
	let open = $state<Record<string, boolean>>({ name: true, size: true, storage: true, apps: true, idle: false });
	let error = $state('');
	let busy = $state(false);

	const order = ['small', 'medium', 'large'];
	const plan = $derived(catalog?.plans.find((p: { id: string }) => p.id === (session.me?.account.trial_plan ?? session.me?.account.plan))?.plan);
	const allowed = (s: string) => !plan || order.indexOf(s) <= order.indexOf(plan.largest_size);
	const needs = (s: string) => catalog.plans.find((p: { plan: { largest_size: string } }) => order.indexOf(p.plan.largest_size) >= order.indexOf(s))?.plan.name;
	const z = $derived(catalog?.sizes[size]);
	const disks = $derived(z ? [z.disk_gb, z.disk_gb * 2, z.disk_gb * 4] : []);
	const hourly = $derived(z ? z.provider_hourly * (1 + catalog.markup) : 0);
	const asleep = $derived(catalog ? disk * catalog.disk_gb_month * (1 + catalog.markup) : 0);
	// Light use: about two hours awake a day, plus storage.
	const light = $derived(hourly * 60 + asleep);
	const usd = (n: number) => `$${n < 0.1 ? n.toFixed(3) : n.toFixed(2)}`;
	const sleeps = [
		{ secs: 30, label: '30 seconds' },
		{ secs: 300, label: '5 minutes' },
		{ secs: 900, label: '15 minutes' },
		{ secs: 3600, label: '1 hour' }
	];
	const sleepWords = $derived(sleeps.find((s) => s.secs === sleepAfter)?.label ?? `${sleepAfter} seconds`);
	const appNames: Record<string, string> = { watcher: 'Watcher', scripts: 'Scripts', code: 'Code' };
	const chosenApps = $derived(Object.keys(apps).filter((a) => apps[a]));
	const appWords = $derived(chosenApps.length ? chosenApps.map((a) => appNames[a]).join(chosenApps.length === 2 ? ' and ' : ', ') : 'just Files');

	onMount(async () => {
		catalog = await get('/plans');
		disk = catalog.sizes[size].disk_gb;
	});
	function pickSize(s: string) {
		if (!allowed(s)) return;
		size = s;
		disk = catalog.sizes[s].disk_gb;
	}

	async function create() {
		busy = true;
		error = '';
		try {
			const c = await post('/computers', { name, size, disk_gb: disk, apps: chosenApps, sleep_delay_secs: sleepAfter, wake_for_schedule: wakeForSchedule });
			setComputer(c.id);
			await refreshMe();
			goto('/home');
		} catch (e) {
			error = message(e);
		} finally {
			busy = false;
		}
	}
</script>

{#snippet head(key: string, n: number, title: string, sub: string, done: boolean)}
	<button class="sec-head" onclick={() => (open[key] = !open[key])} aria-expanded={open[key]}>
		<span class="num" class:done>{#if done}<Icon name="check" size={14} stroke={2.2} />{:else}{n}{/if}</span>
		<span class="sec-t"><span class="h">{title}</span><span class="sub">{sub}</span></span>
		<span class="chev"><Icon name={open[key] ? 'chevron-down' : 'chevron-right'} size={16} /></span>
	</button>
{/snippet}

<AppWindow icon="server" title="New computer">
	{#snippet actions()}<Button variant="quiet" icon="x" onclick={() => history.back()}>Close</Button>{/snippet}
	<div class="scroll">
		<div class="intro">
			<h1>Set up a new computer</h1>
			<p>A private computer in the cloud. It keeps working when you close this tab.</p>
		</div>
		{#if catalog}
			<div class="cols">
				<div class="steps">
					<section class="sec">
						{@render head('name', 1, 'Name', name.trim() || 'What to call it', !!name.trim())}
						{#if open.name}<div class="sec-body"><input bind:value={name} aria-label="Computer name" placeholder="Side projects" maxlength="40" /></div>{/if}
					</section>
					<section class="sec">
						{@render head('size', 2, 'Size', 'How much power it has. You can change this later.', false)}
						{#if open.size}
							<div class="sec-body">
								<div class="sizes">
									{#each order as s (s)}
										{@const k = catalog.sizes[s]}
										<button class="size" class:on={size === s} disabled={!allowed(s)} onclick={() => pickSize(s)} data-size={s} aria-pressed={size === s}>
											<span class="st"><span>{k.label}</span><span class="radio" class:on={size === s}></span></span>
											<span class="mono spec">{k.cpu} CPU · {k.memory_gb} GB</span>
											<span class="suits">{k.suits}</span>
											<span class="mono rate">{allowed(s) ? `${usd(k.provider_hourly * (1 + catalog.markup))} / hr awake` : `On ${needs(s)} and up`}</span>
										</button>
									{/each}
								</div>
							</div>
						{/if}
					</section>
					<section class="sec">
						{@render head('storage', 3, 'Storage', 'Space for files. Kept even while asleep.', false)}
						{#if open.storage}
							<div class="sec-body pills" role="radiogroup" aria-label="Storage">
								{#each disks as d (d)}
									<button class="pill" class:on={disk === d} role="radio" aria-checked={disk === d} onclick={() => (disk = d)}>{d} GB</button>
								{/each}
							</div>
						{/if}
					</section>
					<section class="sec">
						{@render head('apps', 4, 'Apps to start with', 'Pick a few now. Add more anytime from Get apps.', false)}
						{#if open.apps}
							<div class="sec-body apps">
								<div class="app on fixed"><span class="at"><Icon name="folder" size={15} /></span><span class="an">Files</span><span class="always mono">Always</span></div>
								{#each ['watcher', 'scripts', 'code'] as a (a)}
									<label class="app" class:on={apps[a]}>
										<span class="at"><Icon name={a === 'watcher' ? 'eye' : a === 'scripts' ? 'code' : 'braces'} size={15} /></span>
										<span class="an">{appNames[a]}</span>
										<input type="checkbox" bind:checked={apps[a]} />
									</label>
								{/each}
								{#each [['Data', 'data'], ['Fetcher', 'download']] as [n, ic] (n)}
									<div class="app soon" title="{n} is coming soon"><span class="at"><Icon name={ic} size={15} /></span><span class="an">{n}</span><span class="always mono">Soon</span></div>
								{/each}
							</div>
						{/if}
					</section>
					<section class="sec">
						{@render head('idle', 5, 'When nothing is running', `Sleep after ${sleepWords}, ${wakeForSchedule ? 'wake for scheduled work' : "don't wake for scheduled work"}`, false)}
						{#if open.idle}
							<div class="sec-body idle">
								<label class="kv"><span>Sleep after nothing has run for</span>
									<select bind:value={sleepAfter} aria-label="Sleep after">{#each sleeps as s (s.secs)}<option value={s.secs}>{s.label}</option>{/each}</select>
								</label>
								<label class="kv"><span>Wake by itself for scheduled work<span class="sub">Watches and scheduled scripts wake it; off, they wait until you open it.</span></span>
									<input type="checkbox" class="toggle" bind:checked={wakeForSchedule} aria-label="Wake for scheduled work" />
								</label>
							</div>
						{/if}
					</section>
				</div>
				<aside class="side">
					<div class="panel" data-testid="plain-words">
						<span class="label">In plain words</span>
						<p class="words">
							A <strong>{z?.label}</strong> computer called <strong>{name.trim() || 'Untitled'}</strong>, with <strong>{disk} GB</strong> of storage and
							<strong>{appWords}</strong> ready to go. It <strong>sleeps after {sleepWords}</strong> of nothing running{wakeForSchedule ? ' and wakes up by itself for anything scheduled' : ''}.
						</p>
					</div>
					<div class="panel">
						<span class="label">What it costs</span>
						<div class="cost"><span>While awake</span><span class="mono">{usd(hourly)} / hr</span></div>
						<div class="cost"><span>While asleep</span><span class="mono">{usd(asleep)} / mo</span></div>
						<div class="rule"></div>
						<div class="cost strong"><span>Light use estimate</span><span class="mono">~{usd(light)} / mo</span></div>
						<p class="note">About two hours awake a day. Comes out of your free usage first{plan ? `; ${plan.name} includes ${plan.computers === 1 ? 'one computer' : `${plan.computers} computers`}, up to ${cap(plan.largest_size)}` : ''}.</p>
					</div>
					{#if error}<div class="error">{error}</div>{/if}
					<div class="go">
						<Button variant="primary" size="lg" icon="plus" block onclick={create} {busy} disabled={!name.trim()}>Create computer</Button>
						<Button size="lg" onclick={() => history.back()}>Cancel</Button>
					</div>
					<p class="note">Ready in a few seconds. You’ll see it wake up.</p>
				</aside>
			</div>
		{/if}
	</div>
</AppWindow>

<style>
	.scroll {
		flex: 1;
		overflow: auto;
		padding: 28px 28px 40px;
	}
	.intro {
		display: flex;
		flex-direction: column;
		gap: 2px;
		margin-bottom: 20px;
	}
	h1 {
		font-size: 22px;
		line-height: 30px;
		font-weight: 600;
	}
	.intro p {
		font-size: 14px;
		color: var(--mid);
	}
	.cols {
		display: grid;
		grid-template-columns: minmax(0, 1fr) 360px;
		gap: 20px;
		align-items: start;
	}
	.steps {
		display: flex;
		flex-direction: column;
		gap: 10px;
	}
	.sec {
		border: 1px solid var(--line);
		border-radius: 12px;
		background: var(--pane);
	}
	.sec-head {
		width: 100%;
		display: flex;
		align-items: center;
		gap: 14px;
		padding: 16px 18px;
		background: none;
		border: 0;
		color: var(--ink);
		font: inherit;
		text-align: left;
		cursor: pointer;
	}
	.num {
		width: 24px;
		height: 24px;
		border-radius: 9999px;
		background: var(--accent);
		color: var(--on-accent);
		font: 600 12px 'JetBrains Mono', ui-monospace, monospace;
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
	}
	.num.done {
		background: var(--accent-soft);
		color: var(--accent-ink);
	}
	.sec-t {
		flex: 1;
		display: flex;
		flex-direction: column;
	}
	.h {
		font-size: 16px;
		font-weight: 600;
	}
	.sub {
		font-size: 13px;
		color: var(--mid);
		font-weight: 400;
	}
	.chev {
		color: var(--mid);
		display: flex;
	}
	.sec-body {
		padding: 0 18px 18px 56px;
	}
	.sec-body input:not([type='checkbox']) {
		max-width: 420px;
	}
	.sizes {
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		gap: 10px;
	}
	.size {
		display: flex;
		flex-direction: column;
		gap: 6px;
		padding: 14px;
		border-radius: 10px;
		border: 1px solid var(--line);
		background: var(--surface);
		color: var(--ink);
		font: inherit;
		text-align: left;
		cursor: pointer;
	}
	.size.on {
		border-color: var(--accent);
		background: var(--accent-soft);
	}
	.size:disabled {
		opacity: 0.55;
		cursor: default;
	}
	.st {
		display: flex;
		justify-content: space-between;
		align-items: center;
		font-size: 16px;
		font-weight: 600;
	}
	.radio {
		width: 18px;
		height: 18px;
		border-radius: 9999px;
		border: 1.5px solid var(--line-strong);
	}
	.radio.on {
		border: 5px solid var(--accent);
	}
	.spec,
	.rate {
		font-size: 12px;
		color: var(--mid);
	}
	.rate {
		color: var(--ink);
		margin-top: auto;
	}
	.suits {
		font-size: 13px;
		line-height: 18px;
		color: var(--mid);
	}
	.pills {
		display: flex;
		flex-wrap: wrap;
		gap: 8px;
	}
	.pill {
		height: 34px;
		padding: 0 14px;
		border-radius: 6px;
		border: 1px solid var(--line-strong);
		background: var(--surface);
		color: var(--ink);
		font: 500 14px Inter, system-ui, sans-serif;
		cursor: pointer;
	}
	.pill.on {
		background: var(--accent);
		border-color: var(--accent);
		color: var(--on-accent);
	}
	.apps {
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		gap: 10px;
	}
	.app {
		display: flex;
		align-items: center;
		gap: 10px;
		height: 48px;
		padding: 0 12px;
		border-radius: 8px;
		border: 1px solid var(--line);
		background: var(--surface);
		font-size: 14px;
		font-weight: 500;
		cursor: pointer;
	}
	.app.on {
		border-color: var(--accent);
	}
	.app.fixed,
	.app.soon {
		cursor: default;
	}
	.app.soon {
		opacity: 0.55;
	}
	.at {
		width: 28px;
		height: 28px;
		border-radius: 6px;
		background: var(--raised);
		display: flex;
		align-items: center;
		justify-content: center;
	}
	.an {
		flex: 1;
	}
	.always {
		font-size: 10px;
		letter-spacing: 0.06em;
		text-transform: uppercase;
		color: var(--low);
	}
	.app input {
		width: 18px;
		height: 18px;
	}
	.idle {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}
	.kv {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 16px;
		font-size: 14px;
	}
	.kv > span {
		display: flex;
		flex-direction: column;
	}
	.kv select {
		width: 180px;
	}
	.toggle {
		width: 18px;
		height: 18px;
	}
	.side {
		position: sticky;
		top: 0;
		display: flex;
		flex-direction: column;
		gap: 12px;
	}
	.panel {
		border: 1px solid var(--line);
		border-radius: 12px;
		background: var(--pane);
		padding: 18px;
		display: flex;
		flex-direction: column;
		gap: 10px;
	}
	.words {
		font-size: 17px;
		line-height: 26px;
	}
	.words strong {
		font-weight: 600;
	}
	.cost {
		display: flex;
		justify-content: space-between;
		font-size: 14px;
		color: var(--mid);
	}
	.cost .mono {
		color: var(--ink);
		font-size: 13px;
	}
	.cost.strong {
		color: var(--ink);
	}
	.rule {
		height: 1px;
		background: var(--line);
	}
	.note {
		font-size: 12px;
		line-height: 17px;
		color: var(--mid);
	}
	.go {
		display: flex;
		gap: 8px;
	}
	@media (max-width: 1100px) {
		.cols {
			grid-template-columns: minmax(0, 1fr);
		}
		.side {
			position: static;
		}
	}
	@media (max-width: 700px) {
		.sizes,
		.apps {
			grid-template-columns: minmax(0, 1fr);
		}
		.sec-body {
			padding-left: 18px;
		}
	}
</style>
