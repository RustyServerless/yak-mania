<script>
	import { player, register_player, update_player_name } from '$lib/player.svelte.js';
	import { game } from '$lib/game.svelte.js';
	import { ValidatedValue } from '$lib/validator.svelte.js';
	import CModal from './CModal.svelte';
	import yakIcon from '$lib/assets/yak-mania-icon.webp';

	let { show_register = $bindable(false) } = $props();

	let show_edit_name = $state(false);
	let in_operation = $state(false);
	let balance_pop = $state(false);
	let prev_balance = $state(undefined);
	let register_input;

	let player_name = new ValidatedValue([
		(v) => (v && v.length >= 3) || 'At least 3 characters',
		(v) => (v && v.length <= 30) || 'Max 30 characters'
	]);

	let current_player = $derived(game.players.get(player.id));

	$effect(() => {
		if (show_register) {
			setTimeout(() => register_input?.focus(), 50);
		}
	});

	$effect(() => {
		if (!current_player) return;
		const bal = current_player.balance;
		if (prev_balance !== undefined && bal !== prev_balance) {
			balance_pop = true;
			setTimeout(() => {
				balance_pop = false;
			}, 300);
		}
		prev_balance = bal;
	});

	async function handle_register() {
		if (player_name.in_error || player_name.is_empty) return;
		in_operation = true;
		try {
			await register_player(player_name.value.trim());
			show_register = false;
			player_name.reset();
		} finally {
			in_operation = false;
		}
	}

	async function handle_update_name() {
		if (player_name.in_error || player_name.is_empty) return;
		in_operation = true;
		try {
			await update_player_name(player_name.value.trim());
			show_edit_name = false;
			player_name.reset();
		} finally {
			in_operation = false;
		}
	}
</script>

<div class="grid grid-cols-3 grid-rows-2 bg-base-200 p-1">
	<!-- Start: Icon + "Yak mania" -->
	<div class="row-span-2 flex items-center gap-1">
		<img src={yakIcon} alt="Yak Mania" class="h-12 w-12 rounded" />
		<div class="text-md text-left leading-tight font-bold">
			<div>Yak</div>
			<div>mania</div>
		</div>
	</div>

	<!-- Middle: Name + Balance (growing) -->

	{#if player.id && current_player}
		<div class="col-span-2 col-start-2 row-start-1 flex items-center gap-1 justify-self-end">
			<span class="font-semibold">{current_player.name}</span>
			<button
				title="edit_player_name"
				class="btn btn-ghost btn-xs"
				onclick={() => {
					player_name.value = current_player.name;
					show_edit_name = true;
				}}
			>
				<svg xmlns="http://www.w3.org/2000/svg" class="h-4 w-4" viewBox="0 0 24 24">
					<path
						fill="currentColor"
						d="M20.71,7.04C21.1,6.65 21.1,6 20.71,5.63L18.37,3.29C18,2.9 17.35,2.9 16.96,3.29L15.12,5.12L18.87,8.87M3,17.25V21H6.75L17.81,9.93L14.06,6.18L3,17.25Z"
					/>
				</svg>
			</button>
		</div>
		<div
			class="col-start-2 row-start-2 justify-self-center text-xl font-bold transition-[scale] duration-300"
			class:scale-200={balance_pop}
		>
			💵 ${current_player.balance.toFixed(0)}
		</div>
	{/if}
</div>

<CModal bind:open={show_register} id="register_player">
	<h3 class="font-bold">Join the Game</h3>
	<input
		id="player_name"
		bind:this={register_input}
		class="input-bordered input mt-2 w-full"
		placeholder="Your name"
		bind:value={player_name.value}
		onfocusout={player_name.display_error_now}
		onkeydown={(e) => {
			if (e.key === 'Enter' && !in_operation) {
				handle_register();
			}
		}}
	/>
	{#if player_name.display_error}
		<span class="text-xs text-error">{player_name.error}</span>
	{/if}
	<button
		class="btn mt-2 btn-primary"
		onclick={handle_register}
		disabled={in_operation || player_name.is_empty || player_name.in_error}
	>
		{#if in_operation}
			<span class="loading loading-sm loading-spinner"></span>
		{:else}
			Register
		{/if}
	</button>
</CModal>

<CModal bind:open={show_edit_name} id="change_name">
	<h3 class="font-bold">Change Name</h3>
	<input
		id="player_name"
		class="input-bordered input mt-2 w-full"
		placeholder="New name"
		bind:value={player_name.value}
		onfocusout={player_name.display_error_now}
		onkeydown={(e) => {
			if (e.key === 'Enter' && !in_operation) {
				handle_update_name();
			}
		}}
	/>
	{#if player_name.display_error}
		<span class="text-xs text-error">{player_name.error}</span>
	{/if}
	<button
		class="btn mt-2 btn-primary"
		onclick={handle_update_name}
		onkeydown={(e) => {
			if (e.key === 'Enter' && !in_operation) {
				handle_update_name();
			}
		}}
		disabled={in_operation || player_name.is_empty || player_name.in_error}
	>
		{#if in_operation}
			<span class="loading loading-sm loading-spinner"></span>
		{:else}
			Update
		{/if}
	</button>
</CModal>
