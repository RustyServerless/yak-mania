<script>
	import '../app.css';
	import AlertDisplay from '$lib/components/AlertDisplay.svelte';

	let { children } = $props();
</script>

<AlertDisplay />
{@render children()}
