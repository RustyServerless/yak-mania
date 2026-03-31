<script>
	import { onMount, onDestroy } from 'svelte';
	import {
		game,
		get_appsync_admin_client,
		load_game_state,
		subscribe_admin_game_updates,
		unsubscribe_all
	} from '$lib/game.svelte.js';
	import { sign_out } from '$lib/auth.svelte.js';
	import { alert_success, alert_appsync_error } from '$lib/alerts.svelte.js';
	import { resolve } from '$app/paths';
	import { SvelteSet } from 'svelte/reactivity';

	const admin_client = get_appsync_admin_client();

	let in_operation = $state(false);
	let removing = new SvelteSet();

	onMount(() => {
		load_game_state(get_appsync_admin_client());
		subscribe_admin_game_updates();
	});

	onDestroy(() => {
		unsubscribe_all();
	});

	function game_status_mutation(mutation_name) {
		return `
      		mutation {
     			${mutation_name} {
                    sampled
      				game_status
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
     	`;
	}

	let sorted_players = $derived(
		[...game.players.values()].sort((a, b) => {
			return a.name.localeCompare(b.name);
		})
	);

	async function handle_start() {
		in_operation = true;
		try {
			await admin_client.graphql({
				query: game_status_mutation('startGame')
			});
			alert_success('Game started');
		} catch (e) {
			alert_appsync_error(e, 'Could not start game');
		} finally {
			in_operation = false;
		}
	}

	async function handle_stop() {
		in_operation = true;
		try {
			await admin_client.graphql({
				query: game_status_mutation('stopGame')
			});
			alert_success('Game stopped');
		} catch (e) {
			alert_appsync_error(e, 'Could not stop game');
		} finally {
			in_operation = false;
		}
	}

	async function handle_reset() {
		in_operation = true;
		try {
			await admin_client.graphql({
				query: game_status_mutation('resetGame')
			});
			alert_success('Game reset');
		} catch (e) {
			alert_appsync_error(e, 'Could not reset game');
		} finally {
			in_operation = false;
		}
	}

	async function handle_remove(player_id) {
		removing.add(player_id);
		try {
			await admin_client.graphql({
				query: `
					mutation RemovePlayer($player_id: ID!) {
						removePlayer(player_id: $player_id) {
							id
						}
					}
				`,
				variables: { player_id }
			});
			alert_success('Player removed');
		} catch (e) {
			alert_appsync_error(e, 'Could not remove player');
		} finally {
			removing.delete(player_id);
		}
	}
</script>

<div class="mx-auto max-w-4xl p-4">
	<div class="flex items-center justify-between">
		<h1 class="text-2xl font-bold">Yak Mania Admin</h1>
		<button class="btn btn-ghost btn-sm" onclick={sign_out}>Sign Out</button>
	</div>

	<div class="my-4">
		<p>Game Status: <b>{game.status ?? 'Loading...'}</b></p>
		<div class="mt-2 flex gap-2 align-middle">
			<button
				class="btn btn-success"
				disabled={game.status !== 'RESET' || in_operation}
				onclick={handle_start}
			>
				Start
			</button>

			<button
				class="btn btn-warning"
				disabled={game.status !== 'STARTED' || in_operation}
				onclick={handle_stop}
			>
				Stop
			</button>
			<button
				class="btn btn-error"
				disabled={game.status !== 'STOPPED' || in_operation}
				onclick={handle_reset}
			>
				Reset
			</button>
		</div>
	</div>

	<a href={resolve('/admin/dashboard')} class="btn btn-outline btn-sm">View Dashboard</a>

	<div class="mt-4">
		<h2 class="text-lg font-semibold">Players ({game.players.size})</h2>
		<table class="table mt-2">
			<thead>
				<tr>
					<th>Name</th>
					<th>Balance</th>
					<th>Job</th>
					<th></th>
				</tr>
			</thead>
			<tbody>
				{#each sorted_players as p (p.id)}
					<tr>
						<td>{p.name}</td>
						<td>${p.balance.toFixed(0)}</td>
						<td>{p.assignment?.job ?? '-'}</td>
						<td>
							<button
								class="btn btn-xs btn-error"
								disabled={removing.has(p.id)}
								onclick={() => handle_remove(p.id)}
							>
								{#if removing.has(p.id)}
									<span class="loading loading-xs loading-spinner"></span>
								{:else}
									Remove
								{/if}
							</button>
						</td>
					</tr>
				{/each}
			</tbody>
		</table>
	</div>
</div>
