<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import Mark from '$lib/shell/Mark.svelte';
	import Icon from '$lib/ui/Icon.svelte';
	import { get, post, message } from '$lib/api';
	import { refreshMe, session, setComputer } from '$lib/session.svelte';
	import { cap } from '$lib/plans';
	import { disconnectLive } from '$lib/live';

	// First run: no computers yet. Pick a starting point; "Start from scratch" opens the
	// full form. Sizes and prices are the catalog's.
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	let catalog = $state<any>(null);
	let tip = $state<string | null>(null);
	let busy = $state('');
	let error = $state('');
	let menu = $state(false);

	const order = ['small', 'medium', 'large'];
	const me = $derived(session.me);
	const plan = $derived(catalog?.plans.find((p: { id: string }) => p.id === (me?.account.trial_plan ?? me?.account.plan))?.plan);
	const allowed = (s: string) => !plan || order.indexOf(s) <= order.indexOf(plan.largest_size);
	const initials = $derived(((me?.user.name || me?.user.email || '?').split(/[\s@._-]+/).filter(Boolean).slice(0, 2).map((w) => w[0]).join('') || '?').toUpperCase());

	onMount(async () => {
		try {
			const m = await refreshMe();
			if (m.signup_step !== 'done') return goto('/signup', { replaceState: true });
			if (m.computers.length) return goto('/home', { replaceState: true });
			catalog = await get('/plans');
		} catch {
			goto('/signin');
		}
	});

	const hourly = (s: string) => catalog.sizes[s].provider_hourly * (1 + catalog.markup);
	const asleep = (s: string) => catalog.sizes[s].disk_gb * catalog.disk_gb_month * (1 + catalog.markup);
	const usd = (n: number) => `$${n < 0.1 ? n.toFixed(3) : n.toFixed(2)}`;
	const needs = (s: string) => catalog.plans.find((p: { plan: { largest_size: string } }) => order.indexOf(p.plan.largest_size) >= order.indexOf(s))?.plan.name;

	async function start(size: string) {
		if (!allowed(size) || busy) return;
		busy = size;
		error = '';
		try {
			const c = await post('/computers', { name: 'My computer', size });
			setComputer(c.id);
			await refreshMe();
			goto('/home');
		} catch (e) {
			error = message(e);
			busy = '';
		}
	}
	async function signOut() {
		await post('/auth/signout');
		disconnectLive();
		session.me = null;
		goto('/signin');
	}
</script>

<svelte:head><title>Set up your first computer · Croncave</title></svelte:head>

<div class="first">
	<header>
		<div class="brand"><Mark /><span>Croncave</span></div>
		<div class="acct">
			<button class="avatar" aria-label="Account" aria-expanded={menu} onclick={() => (menu = !menu)}>{initials}</button>
			{#if menu}<div class="menu"><button onclick={signOut}>Sign out</button></div>{/if}
		</div>
	</header>
	<main>
		<div class="inner">
			<div class="head">
				<h1>Set up your first computer</h1>
				<p>A private computer in the cloud that keeps working when you close this tab. Pick a starting point. You can change its size and apps anytime.</p>
			</div>
			{#if catalog}
				<div class="cards">
					{#each order as s (s)}
						{@const z = catalog.sizes[s]}
						<div class="slot">
							<button class="card" class:locked={!allowed(s)} data-size={s} onclick={() => start(s)} disabled={!!busy} aria-describedby="tip-{s}">
								<span class="t">{z.label}{#if busy === s}<span class="spin" aria-hidden="true"></span>{/if}</span>
								<span class="d">{z.suits}.</span>
								<span class="spec"><span class="mono">{z.cpu} CPU · {z.memory_gb} GB</span>{#if !allowed(s)}<span class="lock">On {needs(s)} and up</span>{/if}</span>
							</button>
							<button
								class="info"
								aria-label="More about {z.label}"
								onmouseenter={() => (tip = s)}
								onmouseleave={() => (tip = null)}
								onfocus={() => (tip = s)}
								onblur={() => (tip = null)}><Icon name="info" size={16} /></button
							>
							{#if tip === s}
								<div role="tooltip" id="tip-{s}" class="tip">
									<span class="tt">{z.label} computer</span>
									<div class="kv"><span>Processor</span><span class="mono">{z.cpu} CPU</span></div>
									<div class="kv"><span>Memory</span><span class="mono">{z.memory_gb} GB</span></div>
									<div class="kv"><span>Storage</span><span class="mono">{z.disk_gb} GB</span></div>
									<div class="kv"><span>While awake</span><span class="mono">{usd(hourly(s))} / hr</span></div>
									<div class="kv"><span>While asleep</span><span class="mono">{usd(asleep(s))} / mo</span></div>
									<div class="rule"></div>
									<span class="good">Good for: {z.suits.toLowerCase()}.</span>
								</div>
							{/if}
						</div>
					{/each}
				</div>
				{#if error}<div class="error">{error}</div>{/if}
				<span class="scratch">Know exactly what you need? <a href="/computers/new">Start from scratch</a></span>
				{#if plan}<span class="low small">Your {plan.name} plan includes {plan.computers === 1 ? 'one computer' : `${plan.computers} computers`}, up to {cap(plan.largest_size)}.</span>{/if}
			{/if}
		</div>
	</main>
</div>

<style>
	.first {
		min-height: 100vh;
		display: flex;
		flex-direction: column;
		background: var(--bg);
	}
	header {
		height: 60px;
		flex-shrink: 0;
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 0 20px;
	}
	.brand {
		display: flex;
		align-items: center;
		gap: 10px;
		font-size: 15px;
		font-weight: 600;
	}
	.acct {
		position: relative;
	}
	.avatar {
		width: 30px;
		height: 30px;
		border-radius: 9999px;
		background: var(--accent-soft);
		color: var(--accent-ink);
		border: 0;
		font: 600 12px Inter, system-ui, sans-serif;
		cursor: pointer;
	}
	.menu {
		position: absolute;
		right: 0;
		top: 38px;
		background: var(--surface);
		border: 1px solid var(--line);
		border-radius: 10px;
		padding: 6px;
		box-shadow: 0 16px 40px var(--shadow);
		z-index: 5;
	}
	.menu button {
		background: none;
		border: 0;
		color: var(--ink);
		font: 500 14px Inter, system-ui, sans-serif;
		padding: 8px 12px;
		border-radius: 6px;
		cursor: pointer;
		white-space: nowrap;
	}
	.menu button:hover {
		background: var(--pane);
	}
	main {
		flex: 1;
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 0 16px 60px;
	}
	.inner {
		width: 1040px;
		max-width: 100%;
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 32px;
		text-align: center;
	}
	.head {
		display: flex;
		flex-direction: column;
		gap: 10px;
		align-items: center;
	}
	h1 {
		font-size: 32px;
		line-height: 40px;
		font-weight: 600;
		letter-spacing: -0.02em;
	}
	.head p {
		max-width: 560px;
		font-size: 16px;
		line-height: 24px;
		color: var(--mid);
	}
	.cards {
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		gap: 14px;
		width: 820px;
		max-width: 100%;
	}
	.slot {
		position: relative;
		display: flex;
		min-width: 0;
	}
	.card {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
		gap: 10px;
		padding: 18px;
		border-radius: 12px;
		border: 1px solid var(--line);
		background: var(--surface);
		color: var(--ink);
		text-align: left;
		font: inherit;
		cursor: pointer;
	}
	.card:hover:not(.locked) {
		border-color: var(--line-strong);
	}
	.card.locked {
		cursor: default;
		opacity: 0.6;
	}
	.t {
		display: flex;
		align-items: center;
		gap: 8px;
		font-size: 16px;
		font-weight: 600;
	}
	.d {
		font-size: 13px;
		line-height: 19px;
		color: var(--mid);
		flex-grow: 1;
		height: 38px;
		overflow: hidden;
		display: -webkit-box;
		-webkit-line-clamp: 2;
		line-clamp: 2;
		-webkit-box-orient: vertical;
	}
	.spec {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 10px 32px 0 0;
		border-top: 1px solid var(--line);
	}
	.spec .mono {
		font-size: 12px;
		color: var(--mid);
	}
	.lock {
		font-size: 12px;
		color: var(--needs);
	}
	.info {
		position: absolute;
		right: 10px;
		bottom: 8px;
		width: 28px;
		height: 28px;
		padding: 0;
		border: none;
		border-radius: 6px;
		background: transparent;
		color: var(--mid);
		display: flex;
		align-items: center;
		justify-content: center;
		cursor: help;
	}
	.tip {
		position: absolute;
		right: 0;
		bottom: 52px;
		width: 280px;
		padding: 14px;
		border-radius: 10px;
		background: var(--pane);
		border: 1px solid var(--line);
		box-shadow: 0 16px 40px var(--shadow);
		display: flex;
		flex-direction: column;
		gap: 8px;
		text-align: left;
		z-index: 5;
	}
	.tt {
		font-size: 14px;
		font-weight: 600;
	}
	.kv {
		display: flex;
		justify-content: space-between;
		gap: 12px;
		font-size: 13px;
		color: var(--mid);
	}
	.kv .mono {
		font-size: 12px;
		color: var(--ink);
	}
	.rule {
		height: 1px;
		background: var(--line);
	}
	.good {
		font-size: 12px;
		line-height: 17px;
		color: var(--mid);
	}
	.scratch {
		font-size: 14px;
		color: var(--mid);
	}
	.scratch a {
		font-weight: 500;
	}
	.small {
		font-size: 13px;
		margin-top: -20px;
	}
	.spin {
		width: 12px;
		height: 12px;
		border: 2px solid currentColor;
		border-right-color: transparent;
		border-radius: 50%;
		animation: s 0.8s linear infinite;
	}
	@keyframes s {
		to {
			transform: rotate(360deg);
		}
	}
	@media (max-width: 760px) {
		.cards {
			grid-template-columns: minmax(0, 1fr);
		}
	}
</style>
