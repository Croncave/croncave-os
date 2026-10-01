<script lang="ts">
	// The Croncave mark: three bands in the status colors (live, needs you, working).
	let { size = 22 }: { size?: number } = $props();
</script>

<span class="mark" aria-hidden="true" style="width: {size}px; height: {size}px; border-radius: {Math.round(size * 0.27)}px">
	<span style="background: var(--accent)"></span><span style="background: var(--needs)"></span><span style="background: var(--working)"></span>
</span>

<style>
	.mark {
		display: flex;
		flex-direction: column;
		overflow: hidden;
		flex-shrink: 0;
		background: var(--raised);
	}
	.mark span {
		flex: 1;
	}
</style>
