<script lang="ts">
	let { tabs, value = $bindable() }: { tabs: { id: string; label: string }[]; value: string } = $props();
</script>

<div class="tabs" role="tablist">
	{#each tabs as t (t.id)}
		<button role="tab" aria-selected={value === t.id} class:on={value === t.id} onclick={() => (value = t.id)}>{t.label}</button>
	{/each}
</div>

<style>
	.tabs {
		display: flex;
		gap: 4px;
		border-bottom: 1px solid var(--line);
	}
	button {
		background: none;
		border: 0;
		border-bottom: 2px solid transparent;
		color: var(--mid);
		padding: 8px 10px;
		font: inherit;
		cursor: pointer;
	}
	.on {
		color: var(--ink);
		border-bottom-color: var(--accent);
	}
</style>
