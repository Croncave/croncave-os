<script lang="ts">
	import AppWindow from '$lib/shell/AppWindow.svelte';
	import ActivityPanel from '$lib/shell/ActivityPanel.svelte';
</script>

<AppWindow icon="activity" title="Activity" href="/activity" crumbs={[{ label: 'Full history' }]}>
	<div class="wrap"><ActivityPanel full /></div>
</AppWindow>

<style>
	.wrap {
		flex: 1;
		display: flex;
		flex-direction: column;
		min-height: 0;
	}
</style>
