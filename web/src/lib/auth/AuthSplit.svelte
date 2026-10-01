<script lang="ts">
	import type { Snippet } from 'svelte';
	import Icon from '$lib/ui/Icon.svelte';

	// Signed-out screens: the form on the left, and on the right what Croncave does while
	// you're away, shown as the Home "Since you left" card it would fill in.
	let { children, footer = true }: { children: Snippet; footer?: boolean } = $props();

	const example = [
		{ icon: 'terminal', title: 'real-estate-app preview is ready', needs: true, sub: 'The Code agent finished 42 tasks · review its changes', at: '06:12' },
		{ icon: 'eye', title: '2 new apartments matched', sub: 'Apartment watch · sent to your phone and email', at: '10:30' },
		{ icon: 'code', title: 'scrape_jobs.py added 31 rows', sub: 'listings.csv saved to Files › jobs', at: '06:00' },
		{ icon: 'eye', title: 'NVDA crossed $140', sub: 'Stock watch · texted you', at: '04:31' }
	];
</script>

<div class="split">
	<main class="side">
		<div class="brand"><span class="sq" aria-hidden="true"></span><span>Croncave</span></div>
		<div class="center"><div class="form">{@render children()}</div></div>
		{#if footer}
			<p class="legal">By continuing you agree to the Terms and Privacy Policy. Available in the US.</p>
		{:else}<span></span>{/if}
	</main>
	<aside class="pitch" aria-label="What Croncave does while you're away">
		<div class="lede">
			<span class="eyebrow">While you’re away</span>
			<p class="big">Your computer in the cloud, still working after you close the lid.</p>
			<p class="sub">Watch pages and prices, run scripts on a schedule, and hand coding work to an agent. Your computer sleeps when nothing is running, so free usage goes a long way.</p>
		</div>
		<div class="since">
			<div class="since-head"><span class="since-title">Since you left</span><span class="mono low">$0.42 of free usage</span></div>
			<div class="rows">
				{#each example as e, i (i)}
					<div class="r">
						<span class="tile"><Icon name={e.icon} size={18} stroke={2} /></span>
						<div class="what">
							<span class="t">{e.title}{#if e.needs}<span class="pill"><span class="dot"></span>Needs you</span>{/if}</span>
							<span class="s">{e.sub}</span>
						</div>
						<span class="at mono">{e.at}</span>
					</div>
				{/each}
			</div>
		</div>
	</aside>
</div>

<style>
	.split {
		min-height: 100vh;
		display: flex;
		background: var(--bg);
	}
	.side {
		width: 600px;
		flex-shrink: 0;
		padding: 32px 40px;
		display: flex;
		flex-direction: column;
	}
	.brand {
		display: flex;
		align-items: center;
		gap: 10px;
		font-size: 15px;
		line-height: 22px;
		font-weight: 600;
		letter-spacing: -0.01em;
	}
	.sq {
		width: 18px;
		height: 18px;
		border-radius: 4px;
		background: var(--accent);
	}
	.center {
		flex: 1;
		display: flex;
		flex-direction: column;
		justify-content: center;
		align-items: center;
		padding: 40px 0;
	}
	.form {
		width: 380px;
		max-width: 100%;
		display: flex;
		flex-direction: column;
		gap: 24px;
	}
	.legal {
		font-size: 12px;
		line-height: 16px;
		color: var(--low);
	}
	.pitch {
		flex: 1;
		margin: 12px 12px 12px 0;
		padding: 56px;
		border-radius: 12px;
		border: 1px solid var(--line);
		background-color: var(--pane);
		background-image: radial-gradient(var(--line) 1px, transparent 1px);
		background-size: 18px 18px;
		display: flex;
		flex-direction: column;
		justify-content: space-between;
		gap: 40px;
		overflow: hidden;
	}
	.lede {
		display: flex;
		flex-direction: column;
		gap: 14px;
		max-width: 520px;
	}
	.eyebrow {
		font: 500 11px/16px 'JetBrains Mono', ui-monospace, monospace;
		letter-spacing: 0.08em;
		text-transform: uppercase;
		color: var(--accent-ink);
	}
	.big {
		font-size: 36px;
		line-height: 44px;
		font-weight: 600;
		letter-spacing: -0.02em;
	}
	.sub {
		font-size: 16px;
		line-height: 24px;
		color: var(--mid);
	}
	.since {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}
	.since-head {
		display: flex;
		justify-content: space-between;
		align-items: baseline;
		font-size: 12px;
	}
	.since-title {
		font-size: 15px;
		font-weight: 600;
	}
	.rows {
		border-radius: 12px;
		border: 1px solid var(--line);
		background: var(--surface);
		overflow: hidden;
	}
	.r {
		display: flex;
		align-items: center;
		gap: 14px;
		padding: 14px 18px;
	}
	.r + .r {
		border-top: 1px solid var(--line);
	}
	.tile {
		width: 36px;
		height: 36px;
		border-radius: 10px;
		background: var(--raised);
		color: var(--ink);
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
	}
	.what {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
		gap: 2px;
	}
	.t {
		display: flex;
		align-items: center;
		gap: 8px;
		font-size: 15px;
		font-weight: 500;
	}
	.s {
		font-size: 13px;
		color: var(--mid);
	}
	.at {
		font-size: 12px;
		color: var(--low);
		width: 44px;
		text-align: right;
	}
	.pill {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		height: 22px;
		padding: 0 8px;
		border-radius: 4px;
		background: var(--needs-soft);
		color: var(--needs);
		font-size: 12px;
		font-weight: 600;
	}
	.dot {
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background: var(--needs);
	}
	@media (max-width: 1080px) {
		.pitch {
			display: none;
		}
		.side {
			width: 100%;
		}
	}
	@media (max-width: 600px) {
		.side {
			padding: 24px 16px;
		}
	}
</style>
