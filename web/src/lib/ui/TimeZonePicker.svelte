<script lang="ts">
	import { onMount } from 'svelte';
	import { get } from '$lib/api';
	import Icon from './Icon.svelte';

	export type Zone = { id: string; name: string; places: string; short: string; offset: string };
	let { value = $bindable(), label = 'Time zone' }: { value: string; label?: string } = $props();

	let zones = $state<Zone[]>([]);
	let open = $state(false);
	let active = $state(0);
	let root: HTMLDivElement;

	onMount(() => {
		get('/time-zones').then((r) => (zones = r.zones));
	});
	const current = $derived(zones.find((z) => z.id === value));
	const firstPlace = (z: Zone) => z.places.split(',')[0];

	function choose(z: Zone) {
		value = z.id;
		open = false;
		root.querySelector('button')?.focus();
	}
	function toggle() {
		open = !open;
		active = Math.max(0, zones.findIndex((z) => z.id === value));
	}
	function keys(e: KeyboardEvent) {
		if (!open) {
			if (e.key === 'ArrowDown' || e.key === 'Enter' || e.key === ' ') (e.preventDefault(), toggle());
			return;
		}
		if (e.key === 'ArrowDown') (e.preventDefault(), (active = Math.min(zones.length - 1, active + 1)));
		else if (e.key === 'ArrowUp') (e.preventDefault(), (active = Math.max(0, active - 1)));
		else if (e.key === 'Enter' || e.key === ' ') (e.preventDefault(), choose(zones[active]));
		else if (e.key === 'Escape') (e.preventDefault(), (open = false));
	}
</script>

<svelte:window onclick={(e) => open && !root.contains(e.target as Node) && (open = false)} />

<div class="picker" bind:this={root}>
	<button type="button" class="pick" class:open aria-haspopup="listbox" aria-expanded={open} aria-label={label} onclick={toggle} onkeydown={keys}>
		{#if current}<span>{current.name} <span class="mono low">· {firstPlace(current)} · {current.offset}</span></span>{:else}<span class="low">Choose your time zone</span>{/if}
		<span class="chev"><Icon name="chevron-down" size={14} /></span>
	</button>
	{#if open}
		<ul role="listbox" aria-label="US time zones">
			{#each zones as z, i (z.id)}
				<li role="option" aria-selected={z.id === value} class:active={i === active} onclick={() => choose(z)} onkeydown={keys} onmouseenter={() => (active = i)}>
					<span class="tick">{#if z.id === value}<Icon name="check" size={14} stroke={2} />{/if}</span>
					<span class="what"><span class="zn" class:sel={z.id === value}>{z.name}</span><span class="zp">{z.places}</span></span>
					<span class="mono low off">{z.offset}</span>
				</li>
			{/each}
		</ul>
	{/if}
</div>

<style>
	.picker {
		position: relative;
	}
	.pick {
		width: 100%;
		height: 42px;
		padding: 0 12px;
		border: 1px solid var(--line-strong);
		border-radius: 6px;
		background: var(--pane);
		color: var(--ink);
		font: 14px Inter, system-ui, sans-serif;
		display: flex;
		align-items: center;
		justify-content: space-between;
		cursor: pointer;
		text-align: left;
	}
	.pick.open {
		border: 2px solid var(--working);
		padding: 0 11px;
	}
	.pick .mono {
		font-size: 12px;
		color: var(--mid);
	}
	.chev {
		color: var(--low);
		display: flex;
	}
	ul {
		position: absolute;
		left: 0;
		right: 0;
		top: 48px;
		margin: 0;
		padding: 6px;
		list-style: none;
		background: var(--surface);
		border: 1px solid var(--line);
		border-radius: 10px;
		box-shadow: 0 16px 40px var(--shadow);
		display: flex;
		flex-direction: column;
		gap: 1px;
		z-index: 5;
		max-height: 420px;
		overflow: auto;
	}
	li {
		display: grid;
		grid-template-columns: 14px minmax(0, 1fr) auto;
		gap: 10px;
		align-items: center;
		padding: 7px 10px;
		border-radius: 6px;
		cursor: pointer;
	}
	li.active {
		background: var(--raised);
	}
	.tick {
		color: var(--accent-ink);
		display: flex;
	}
	.what {
		display: flex;
		flex-direction: column;
		min-width: 0;
	}
	.zn {
		font-size: 13px;
		font-weight: 500;
	}
	.zn.sel {
		font-weight: 600;
	}
	.zp {
		font-size: 12px;
		color: var(--mid);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.off {
		font-size: 12px;
	}
</style>
