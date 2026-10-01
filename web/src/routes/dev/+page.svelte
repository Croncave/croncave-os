<script lang="ts">
	import { onMount } from 'svelte';
	import Brand from '../Brand.svelte';
	import Button from '$lib/ui/Button.svelte';
	import { get, post, message } from '$lib/api';
	import { toast } from '$lib/ui/toast.svelte';
	import { when } from '$lib/format';

	// Dev tools: read the simulated email and texts, move time, and poke the demo sites.
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	let messages = $state<any[]>([]);
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	let info = $state<any>(null);
	let error = $state('');

	async function load() {
		try {
			messages = (await get('/dev/outbox')).messages;
			info = await get('/dev/state');
		} catch (e) {
			error = message(e);
		}
	}
	onMount(() => {
		load();
		const t = setInterval(load, 2000);
		return () => clearInterval(t);
	});

	async function act(path: string, body: unknown, done: string) {
		try {
			await post(path, body);
			toast(done);
			load();
		} catch (e) {
			toast(message(e), true);
		}
	}
	const advance = (secs: number, label: string) => act('/dev/clock', { secs }, `Moved time forward ${label}`);
	const codeIn = (body: string) => body.match(/\b\d{6}\b/)?.[0];
</script>

<main class="page" style="margin: 0 auto">
	<div class="page-head"><Brand /><a href="/">Back to Croncave</a></div>
	<h1>Dev tools</h1>
	{#if error}<div class="banner bad">{error} (Dev tools need DEV_TOOLS=true.)</div>{/if}
	<div class="grid2">
		<section class="card stack">
			<h2>Time</h2>
			<p class="mid">Now: <span class="mono">{when(info?.now)}</span>{#if info?.clock_offset_secs} <span class="chip">moved {Math.round(info.clock_offset_secs / 3600)}h ahead</span>{/if}</p>
			<p class="low">Business time (trials, months, schedules) moves; sign-ins and billed awake time don't.</p>
			<div class="row">
				<Button size="sm" onclick={() => advance(3600, '1 hour')}>+1 hour</Button>
				<Button size="sm" onclick={() => advance(86400, '1 day')}>+1 day</Button>
				<Button size="sm" onclick={() => advance(7 * 86400, '7 days')}>+7 days</Button>
				<Button size="sm" onclick={() => advance(15 * 86400, '15 days')}>+15 days (ends a Free trial)</Button>
				<Button size="sm" onclick={() => advance(31 * 86400, '31 days')}>+31 days (closes a month)</Button>
			</div>
		</section>
		<section class="card stack">
			<h2>Demo sites and data</h2>
			<p class="mid">{info?.listings ?? '…'} listings on <a href="/demo/listings" target="_blank">Demo Rentals</a>. The <a href="/demo/page" target="_blank">bakery page</a> says: “{info?.page}”</p>
			<div class="row">
				<Button size="sm" onclick={() => act('/dev/demo/listing', {}, 'Added a listing')}>Add a listing</Button>
				<Button size="sm" onclick={() => act('/dev/demo/page', {}, 'Changed the page')}>Change the bakery page</Button>
				<Button size="sm" onclick={() => act('/dev/demo/stock', { symbol: 'ACME', percent: -10 }, 'ACME down 10%')}>ACME −10%</Button>
				<Button size="sm" onclick={() => act('/dev/demo/stock', { symbol: 'ACME', percent: 10 }, 'ACME up 10%')}>ACME +10%</Button>
			</div>
			<h3>Usage (your account)</h3>
			<div class="row">
				<Button size="sm" onclick={() => act('/dev/usage', { dollars: 0.5 }, 'Added $0.50 of usage')}>+$0.50 usage</Button>
				<Button size="sm" onclick={() => act('/dev/usage', { dollars: 2 }, 'Added $2 of usage')}>+$2 usage</Button>
				<Button size="sm" onclick={() => act('/dev/usage', { dollars: 5 }, 'Added $5 of usage')}>+$5 usage</Button>
			</div>
			{#if info}<p class="low mono">Drivers: {Object.entries(info.drivers).map(([k, v]) => `${k}=${v}`).join(' · ')}</p>{/if}
		</section>
	</div>
	<section class="card stack">
		<h2>Outbox <span class="low">(email, text messages and push, as the Notifier would send them)</span></h2>
		<div class="list" data-testid="outbox">
			{#each messages as m (m.id)}
				<div class="stack" style="gap: 4px">
					<div class="row"><span class="chip">{m.channel}</span><strong>{m.subject}</strong><span class="low">to {m.to}</span><span class="spacer"></span><span class="low mono">{when(m.at)}</span></div>
					{#if m.channel === 'email' && m.link?.includes('/auth/verify')}
						<a href={m.link.replace(/^https?:\/\/[^/]+/, '')} data-testid="signin-link">Open the sign-in link</a>
					{:else if m.channel === 'sms' && codeIn(m.body)}
						<span>Code: <strong class="mono" data-testid="sms-code">{codeIn(m.body)}</strong></span>
					{:else}
						<pre class="body">{m.body}</pre>
					{/if}
				</div>
			{:else}
				<div class="muted-box">Nothing sent yet.</div>
			{/each}
		</div>
	</section>
</main>

<style>
	.body {
		margin: 0;
		white-space: pre-wrap;
		color: var(--mid);
	}
</style>
