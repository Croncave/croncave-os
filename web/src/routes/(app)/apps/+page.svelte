<script lang="ts">
	import { get } from '$lib/api';
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	let apps = $state<any[]>([]);
	$effect(() => {
		get('/apps').then((r) => (apps = r.apps));
	});
</script>

<div class="page">
	<h1>All apps</h1>
	<div class="grid2">
		{#each apps as a (a.id)}
			<a class="card stack app" href={a.id === 'files' ? '/files' : `/${a.id}`}>
				<h2>{a.name}</h2>
				<p class="mid">{a.description}</p>
				<p class="low mono small">Needs: {a.permissions.join(', ')}</p>
			</a>
		{/each}
	</div>
	<section class="card stack" id="get">
		<h2>Get apps</h2>
		<p class="mid">More apps arrive after launch, built on the same platform as these. Each one asks for exactly what it needs before you install it.</p>
	</section>
</div>

<style>
	.app {
		color: var(--ink);
	}
	.app:hover {
		text-decoration: none;
		border-color: var(--accent);
	}
	.small {
		font-size: 11.5px;
	}
</style>
