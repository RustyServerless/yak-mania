<script>
	let { children, open = $bindable(), id, onclose } = $props();
	let was_open = false;
	$effect(() => {
		if (was_open && !open) {
			onclose?.();
		}
		was_open = open;
	});
</script>

<input type="checkbox" {id} class="modal-toggle" bind:checked={open} />
<div class="modal z-40" role="dialog">
	<div class="modal-box">
		{@render children?.()}
	</div>
	<label class="modal-backdrop" for={id}>Close</label>
</div>
