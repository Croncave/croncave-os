<script lang="ts">
	import { page } from '$app/state';
	import { goto } from '$app/navigation';
	import { onChange, onLive, throttle } from '$lib/live';
	import { session } from '$lib/session.svelte';
	import { clock } from '$lib/format';
	import AppWindow from '$lib/shell/AppWindow.svelte';
	import Button from '$lib/ui/Button.svelte';
	import Icon from '$lib/ui/Icon.svelte';
	import Status from '$lib/ui/Status.svelte';
	import { lang, loadScripts, scriptState, scripts } from '$lib/apps/scripts/store.svelte';

	// Scripts: your scripts on the left, the one you're looking at (or adding) on the right.
	let { children } = $props();
	const adding = $derived(page.url.searchParams.has('add'));
	const current = $derived(page.params.job ?? '');

	$effect(() => {
		void session.computerId;
		loadScripts();
	});
	$effect(() => onChange(throttle(loadScripts, 500)));
	// Keep "Running · 62%" moving.
	$effect(() => {
		const refresh = throttle(loadScripts, 3000);
		return onLive((m) => m.kind === 'progress' && refresh());
	});

	function sub(j: (typeof scripts.jobs)[number]) {
		const r = j.last_run;
		if (r && r.status === 'running' && j.trigger === 'manual') return `Run by hand · started ${clock(r.started_at)}`;
		const when = j.trigger === 'schedule' ? (j.schedule_words ?? '') : j.trigger === 'files' ? `When files land in ${j.watch_path || 'your files'}` : 'Run by hand';
		const last = r?.ended_at ? ` · last ran ${clock(r.ended_at)}` : '';
		return when + (j.trigger === 'files' ? '' : last);
	}
</script>

{#if adding}
	<AppWindow icon="code" title="Scripts" href="/scripts" crumbs={[{ label: 'Add a script' }]}>
		{#snippet actions()}<Button variant="quiet" icon="x" onclick={() => (history.length > 1 ? history.back() : goto('/scripts'))}>Close</Button>{/snippet}
		{@render children()}
	</AppWindow>
{:else}
	<AppWindow icon="code" title="Scripts">
		<aside class="slist" aria-label="Your scripts">
			<div class="lh"><span class="lt">Your scripts</span><Button size="sm" icon="plus" href="/scripts?add=1">Add</Button></div>
			<div class="items">
				{#each scripts.jobs as j (j.id)}
					{@const st = scriptState(j)}
					<a class="item" class:on={current === j.id} href="/scripts/{j.id}">
						<span class="top"><span class="nm mono">{j.name}</span><span class="lang mono">{lang(j.setup.path)}</span></span>
						<span class="sub">{sub(j)}</span>
						<Status status={st.status} word={st.word} />
					</a>
				{:else}
					{#if scripts.loaded}<p class="none">No scripts yet.</p>{/if}
				{/each}
			</div>
			<a class="foot" href="/files?path=Scripts">Scripts live in Files <Icon name="chevron-right" size={12} /> <span class="mono">Scripts</span></a>
		</aside>
		<div class="detail">{@render children()}</div>
	</AppWindow>
{/if}

<style>
	.slist {
		width: 296px;
		flex-shrink: 0;
		border-right: 1px solid var(--line);
		display: flex;
		flex-direction: column;
		padding: 16px 12px 16px;
		gap: 10px;
	}
	.lh {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 0 4px;
	}
	.lt {
		font-size: 15px;
		font-weight: 600;
	}
	.items {
		flex: 1;
		overflow: auto;
		display: flex;
		flex-direction: column;
		gap: 4px;
	}
	.item {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 4px;
		padding: 12px 12px;
		border-radius: 10px;
		border: 1px solid transparent;
		color: var(--ink);
	}
	.item:hover {
		background: var(--pane);
		text-decoration: none;
	}
	.item.on {
		background: var(--pane);
		border-color: var(--line);
	}
	.top {
		width: 100%;
		display: flex;
		justify-content: space-between;
		gap: 8px;
	}
	.nm {
		font-size: 14px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.lang {
		font-size: 11px;
		color: var(--low);
	}
	.sub {
		font-size: 13px;
		color: var(--mid);
	}
	.none {
		padding: 8px;
		color: var(--mid);
		font-size: 14px;
	}
	.foot {
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 0 4px;
		font-size: 13px;
		color: var(--mid);
	}
	.foot .mono {
		font-size: 13px;
	}
	.detail {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
		overflow: auto;
	}
	@media (max-width: 900px) {
		.slist {
			width: 220px;
		}
	}
	@media (max-width: 700px) {
		.slist {
			display: none;
		}
	}
</style>
