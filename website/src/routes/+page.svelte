<script>
	import { onMount, onDestroy } from 'svelte';
	import {
		game,
		get_appsync_client,
		load_game_state,
		subscribe_player_game_updates,
		unsubscribe_all
	} from '$lib/game.svelte.js';
	import { player } from '$lib/player.svelte.js';
	import { alert_appsync_error } from '$lib/alerts.svelte.js';
	import PlayerHeader from '$lib/components/PlayerHeader.svelte';
	import JobSelector from '$lib/components/JobSelector.svelte';
	import LoadingOverlay from '$lib/components/LoadingOverlay.svelte';
	import DriverOverlay from '$lib/components/DriverOverlay.svelte';
	import BreederView from '$lib/components/BreederView.svelte';
	import DriverView from '$lib/components/DriverView.svelte';
	import ShearerView from '$lib/components/ShearerView.svelte';
	import babyYak from '$lib/assets/baby-yak-sprite.webp';
	import youngYak from '$lib/assets/young-yak-sprite.webp';
	import hairyYak from '$lib/assets/hairy-yak-sprite.webp';
	import nakedYak from '$lib/assets/naked-yak-sprite.webp';

	let selected_job = $state(null);
	let current_yak = $state(null);
	let job_phase = $state('idle');
	let api_pending = $state(false);
	let show_register = $state(false);

	let current_player = $derived(game.players.get(player.id));

	let has_processable_yaks = $derived.by(() => {
		const c = game.yak_counts;
		if (!c) return false;
		return (
			c.in_nursery > 0 ||
			c.in_warehouse > 0 ||
			c.in_shearingshed > 0 ||
			c.with_breeders > 0 ||
			c.with_drivers > 0
		);
	});

	const BUY_MUTATIONS = {
		BREEDER: 'buyBabyYak',
		DRIVER: 'buyGrownYak',
		SHEARER: 'buyUnshearedYak'
	};

	const SELL_MUTATIONS = {
		BREEDER: 'sellGrownYak',
		DRIVER: 'sellUnshearedYak',
		SHEARER: 'sellShearedYak'
	};

	const BUY_SPRITES = {
		BREEDER: babyYak,
		DRIVER: youngYak,
		SHEARER: hairyYak
	};

	const SELL_SPRITES = {
		BREEDER: hairyYak,
		DRIVER: hairyYak,
		SHEARER: nakedYak
	};

	let overlay_sprite = $derived.by(() => {
		if (!selected_job) return null;
		if (job_phase === 'buying') return BUY_SPRITES[selected_job];
		if (job_phase === 'selling') return SELL_SPRITES[selected_job];
		return null;
	});

	function reset_job() {
		selected_job = null;
		current_yak = null;
		job_phase = 'idle';
		api_pending = false;
	}

	async function handle_job_select(job) {
		selected_job = job;
		job_phase = 'buying';
		api_pending = true;

		const buy_mutation = BUY_MUTATIONS[job];

		let animation_resolve;
		const animation_promise = new Promise((r) => (animation_resolve = r));
		overlay_resolve = animation_resolve;

		const api_promise = get_appsync_client()
			.graphql({
				query: `
					mutation BuyYak($player_id: ID!, $secret: String!) {
						${buy_mutation}(player_id: $player_id, secret: $secret) {
							sampled
							game_status
							player {
								id
								name
								assignment {
									job
									yak {
										id
									}
									fee
								}
								balance
								yak_bred
								yak_driven
								yak_sheared
							}
							yak_counts {
								in_nursery
								with_breeders
								in_warehouse
								with_drivers
								in_shearingshed
								with_shearers
								total_sheared
							}
							job_fees {
								breeder
								driver
								shearer
							}
						}
					}
				`,
				variables: { player_id: player.id, secret: player.secret }
			})
			.then((res) => {
				api_pending = false;
				return res.data;
			});

		try {
			const [data] = await Promise.all([api_promise, animation_promise]);
			current_yak = data[buy_mutation].player.assignment.yak;
			job_phase = 'working';
		} catch (e) {
			alert_appsync_error(e, 'Could not buy yak');
			reset_job();
		}
	}

	async function resume_job(assignment) {
		selected_job = assignment.job;
		job_phase = 'buying';
		api_pending = false;

		const animation_promise = new Promise((r) => (overlay_resolve = r));
		await animation_promise;

		current_yak = assignment.yak;
		job_phase = 'working';
	}

	let overlay_resolve = null;

	function handle_overlay_finished() {
		overlay_resolve?.();
		overlay_resolve = null;
	}

	async function handle_job_complete() {
		job_phase = 'selling';
		api_pending = true;

		const sell_mutation = SELL_MUTATIONS[selected_job];

		let animation_resolve;
		const animation_promise = new Promise((r) => (animation_resolve = r));
		overlay_resolve = animation_resolve;

		const api_promise = get_appsync_client()
			.graphql({
				query: `
					mutation SellYak($player_id: ID!, $secret: String!, $yak_id: ID!) {
						${sell_mutation}(player_id: $player_id, secret: $secret, yak_id: $yak_id) {
							sampled
							game_status
							player {
								id
								name
								assignment {
									job
									yak {
										id
									}
									fee
								}
								balance
								yak_bred
								yak_driven
								yak_sheared
							}
							yak_counts {
								in_nursery
								with_breeders
								in_warehouse
								with_drivers
								in_shearingshed
								with_shearers
								total_sheared
							}
							job_fees {
								breeder
								driver
								shearer
							}
						}
					}
				`,
				variables: { player_id: player.id, secret: player.secret, yak_id: current_yak.id }
			})
			.then((res) => {
				api_pending = false;
				return res.data;
			});

		try {
			await Promise.all([api_promise, animation_promise]);
		} catch (e) {
			alert_appsync_error(e, 'Could not sell yak');
		}

		reset_job();
	}

	onMount(async () => {
		const state = await load_game_state(get_appsync_client());
		subscribe_player_game_updates();

		if (state && player.id) {
			const me = state.players.find((p) => p.id === player.id);
			if (me?.assignment) {
				resume_job(me.assignment);
			}
		}
	});

	onDestroy(() => {
		unsubscribe_all();
	});
</script>

<div class="flex h-dvh flex-col">
	<PlayerHeader bind:show_register />

	<main class="relative flex grow flex-col overflow-hidden">
		{#if job_phase === 'working' || job_phase === 'selling'}
			{#if selected_job === 'BREEDER'}
				<BreederView yak={current_yak} oncomplete={handle_job_complete} />
			{:else if selected_job === 'DRIVER'}
				<DriverView yak={current_yak} oncomplete={handle_job_complete} />
			{:else if selected_job === 'SHEARER'}
				<ShearerView yak={current_yak} oncomplete={handle_job_complete} />
			{/if}
		{:else}
			<div class="flex grow flex-col items-center justify-center gap-4 p-4 text-center">
				{#if !(player.id && current_player)}
					<button class="btn text-2xl btn-lg btn-primary" onclick={() => (show_register = true)}>
						Tap to join
					</button>
				{:else if game.status === 'RESET'}
					<p class="text-2xl font-semibold opacity-70">Waiting for the game to start</p>
					<p class="animate-[hourglass-spin_2000ms_ease-in-out_infinite] text-4xl">⏳</p>
				{:else if game.status === 'STOPPED' && !has_processable_yaks}
					<p class="text-3xl font-bold">Game Over</p>
					<p class="text-2xl opacity-70">No more yaks!</p>
				{:else if game.status === 'STOPPED' && has_processable_yaks}
					<div class="h-1/2"></div>
					<p class="text-2xl font-bold">Last yaks!</p>
					<p class="text-lg">Hurry, grab one before they're gone!</p>
					<div class="grow"></div>
					<p class="animate-bounce text-4xl">
						<svg
							xmlns="http://www.w3.org/2000/svg"
							class="h-24 fill-base-content"
							viewBox="0 0 24 24"
						>
							<path d="M9,4H15V12H19.84L12,19.84L4.16,12H9V4Z" />
						</svg>
					</p>
					<div class="grow"></div>
				{:else}
					<div class="h-1/2"></div>
					<p class="text-2xl font-semibold">Select a job to start playing</p>
					<div class="grow"></div>
					<p class="animate-bounce text-4xl">
						<svg
							xmlns="http://www.w3.org/2000/svg"
							class="h-24 fill-base-content"
							viewBox="0 0 24 24"
						>
							<path d="M9,4H15V12H19.84L12,19.84L4.16,12H9V4Z" />
						</svg>
					</p>
					<div class="grow"></div>
				{/if}
			</div>
		{/if}

		{#if job_phase === 'buying' || job_phase === 'selling'}
			{#if selected_job === 'DRIVER'}
				<DriverOverlay
					direction={job_phase === 'buying' ? 'in' : 'out'}
					waiting={api_pending}
					onfinished={handle_overlay_finished}
				/>
			{:else}
				<LoadingOverlay
					sprite={overlay_sprite}
					direction={job_phase === 'buying' ? 'in' : 'out'}
					waiting={api_pending}
					onfinished={handle_overlay_finished}
				/>
			{/if}
		{/if}
	</main>

	<JobSelector {selected_job} locked={job_phase !== 'idle'} onselect={handle_job_select} />
</div>
