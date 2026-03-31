# Website — Agent Guidelines

## Build & Run Commands

All commands run from `website/`.

```bash
npm install          # Install dependencies
npm run dev          # Dev server on http://localhost:3000
npm run build        # Production build (SSG to build/)
npm run preview      # Preview production build on :3000
npm run lint         # ESLint + Prettier check (must pass before commit)
npm run format       # Auto-fix Prettier formatting
```

There is no test suite. Validation is `npm run lint` (zero errors required) then `npm run build`. Always run validation as a single chained command to avoid unnecessary round-trips:

```bash
npm run format && npm run lint && npm run build
```

## Tech Stack

- **SvelteKit 2 + Svelte 5** — Runes only. No `svelte/store`.
- **TailwindCSS v4 + DaisyUI v5** — configured in `src/app.css`. Themes: `light` (default), `dark` (prefers-dark).
- **`@sveltejs/adapter-static`** — full SSG, output to `build/`.
- **AWS Amplify v6** — Cognito auth (admin), AppSync GraphQL (players + admin).
- **GraphQL schema** — source of truth at `/home/firkraag/git/yak-mania/graphql/schema.gql`.

## Formatting (Prettier)

Configured in `.prettierrc`. Non-negotiable:

- **Tabs** for indentation (not spaces)
- **Single quotes** (not double)
- **No trailing commas**
- **100 char** print width
- Plugins: `prettier-plugin-svelte`, `prettier-plugin-tailwindcss`

Always run `npm run format` after making changes. The `tailwindcss` plugin auto-sorts class order.

## ESLint Rules to Watch

- `svelte/no-navigation-without-resolve` — always wrap `goto()` paths and `href` attributes with `resolve()` from `$app/paths`.
- `svelte/prefer-svelte-reactivity` — use `SvelteMap`/`SvelteSet` from `svelte/reactivity` instead of native `Map`/`Set` in `.svelte` and `.svelte.js` files. Exception: non-reactive buffers (see below) use `// eslint-disable-next-line svelte/prefer-svelte-reactivity`.
- `svelte/no-unnecessary-state-wrap` — `SvelteMap`/`SvelteSet` are already reactive; do not wrap them in `$state()`.

## Svelte 5 Patterns (Mandatory)

### Runes Only

Use `$state()`, `$derived()`, `$derived.by()`, `$effect()`, `$props()`, `$bindable()`. Never import from `svelte/store`.

### Global State in `.svelte.js` Files

Module-level `$state()` exports for shared state (`player`, `game`, `signed_user`, `alerts`).

**Critical:** `$effect()` cannot run at module top-level in `.svelte.js` files (no owning context). Wrap in `$effect.root()`:

```js
if (browser) {
	$effect.root(() => {
		$effect(() => {
			/* reactive logic */
		});
	});
}
```

**Critical:** `$derived` cannot be exported from `.svelte.js` modules. If consumers need derived values, they create their own local `$derived` / `$derived.by()` using reactive state imported from the module:

```js
// In component — derive from imported reactive state:
let sorted_players = $derived.by(() => {
	return [...game.players.values()].sort(/*...*/);
});
```

### `in_operation` Pattern

Every async API call uses a local `$state(false)` boolean, set `true` before the call, `false` in `finally`. Disables buttons/inputs during the call.

### `ValidatedValue` for Text Inputs

All user text inputs use `ValidatedValue` from `$lib/validator.svelte.js` with rule functions returning `true` or an error string.

### No `<style>` Blocks

Tailwind/DaisyUI classes only. Aim for max ~10 classes per element.

## Styling

- **No `<style>` blocks** — Tailwind utility classes only.
- `/` (game view) — mobile-first, fullscreen (`h-dvh flex flex-col`).
- `/admin` — desktop, minimal functional design (`max-w-4xl mx-auto`).
- `/admin/dashboard` — 4:3 monitor, no scrolling, large readable text.

## GraphQL Conventions

- **Inline query strings** — no `.graphql` files, no codegen.
- **Multi-line formatting** — one field per line, cleanly indented:

```js
await client.graphql({
	query: `
		mutation RegisterNewPlayer($name: String!, $secret: String!) {
			registerNewPlayer(name: $name, secret: $secret) {
				player {
					id
					name
				}
			}
		}
	`,
	variables: { name, secret }
});
```

- **Error handling** — always `alert_appsync_error(e, 'Human-readable context')` in catch blocks.

## AppSync Clients

Clients are **lazy-initialized** (to avoid "Amplify not configured" warnings during SSR):

```js
import { get_appsync_client, get_appsync_admin_client } from '$lib/game.svelte.js';
```

- `get_appsync_client()` — API key auth (players). Used for all player mutations and subscriptions.
- `get_appsync_admin_client()` — Cognito `userPool` auth (admin). Used for start/stop/reset/remove mutations.

## Subscription Buffering

High-frequency subscription data writes into **non-reactive** plain JS buffers (`Map`, plain variables — NOT `$state`). A self-managing `setInterval` (250ms) flushes buffers into reactive `$state`. The interval auto-stops when there are no pending changes. This keeps Svelte updates at ~4Hz regardless of event throughput. `game_status` changes and resets bypass buffers and apply immediately.

## Naming Conventions

- **snake_case** for variables, functions, file names: `player_name`, `load_game_state`, `alerts.svelte.js`
- **PascalCase** for Svelte components: `PlayerHeader.svelte`, `CModal.svelte`
- **UPPER_SNAKE_CASE** for constants: `GAME_UPDATED_QUERY`
- File extensions: `.svelte.js` for files using runes, `.js` for plain modules

## Import Order

1. External packages (`aws-amplify/*`, `uuid`, `svelte`, `svelte/reactivity`)
2. SvelteKit modules (`$app/environment`, `$app/navigation`, `$app/paths`)
3. Internal `$lib/` modules
4. Relative component imports (`./CModal.svelte`)

## Files You Must Not Modify

- `src/app.html`
- `src/routes/+layout.js`
- `svelte.config.js`
- `vite.config.js`

## Key Architecture Decisions

- **Player auth**: not Cognito — UUID secret in localStorage, sent with every mutation.
- **Admin auth**: Cognito Hosted UI redirect flow (`signInWithRedirect`), `Admins` group check.
- **Admin routes** (`/admin/*`): hidden, no link from game view. Auth guard in `admin/+layout.svelte`.
- **Player eviction**: handled in `game.svelte.js`'s `removedPlayer` subscription handler (calls `clear_player()`), not at the page level.
- **Player rank**: static placeholder `—` in game view. Dashboard computes ranks inline via `$derived.by()` (sorts by balance descending, ties broken alphabetically).

## Project Folder Organization

```
website/src/
├── app.html                                  # HTML shell (do not modify)
├── app.css                                   # TailwindCSS v4 + DaisyUI v5 theme config + custom keyframes
├── hooks.client.js                           # SvelteKit client init hook
├── lib/
│   ├── amplify.js                            # AWS Amplify configuration
│   ├── alerts.svelte.js                      # Global alert/toast system
│   ├── auth.svelte.js                        # Cognito admin authentication state
│   ├── player.svelte.js                      # Player identity + localStorage persistence
│   ├── game.svelte.js                        # Core game state, subscriptions, buffering
│   ├── validator.svelte.js                   # ValidatedValue form validation class
│   └── components/
│       ├── AlertDisplay.svelte               # Toast notification overlay
│       ├── CModal.svelte                     # Generic reusable modal
│       ├── PlayerHeader.svelte               # Game view top bar (name, balance, registration)
│       ├── JobSelector.svelte                # Game view bottom bar (3 job buttons)
│       ├── LoadingOverlay.svelte              # Generic yak zoom-in/out overlay
│       ├── DriverOverlay.svelte              # Truck enter/exit overlay for driver buy/sell
│       ├── BreederView.svelte                # Breeder mini-game (haystack collection)
│       ├── DriverView.svelte                 # Driver job view (placeholder)
│       └── ShearerView.svelte                # Shearer job view (placeholder)
└── routes/
    ├── +layout.js                            # SSG prerender flag (do not modify)
    ├── +layout.svelte                        # Root layout (AlertDisplay + route content)
    ├── +page.svelte                          # Main game view (mobile, fullscreen)
    └── admin/
        ├── +layout.svelte                    # Cognito auth guard for all admin routes
        ├── +page.svelte                      # Admin control panel (start/stop/reset/remove)
        └── dashboard/
            └── +page.svelte                  # Live dashboard (4:3 monitor, leaderboard)
```

### Source File Descriptions

#### Entry Points

- **`hooks.client.js`** — SvelteKit client-side init hook. Calls `loadAmplify()` before any component renders so that Cognito and AppSync are configured early.

#### `lib/` — Shared Modules

- **`amplify.js`** — Exports `loadAmplify()` which calls `Amplify.configure()` with Cognito OAuth (Hosted UI, PKCE code flow) and AppSync GraphQL (API key default auth). All values come from `VITE_*` env vars.

- **`alerts.svelte.js`** — Global reactive alert system. Exports a `SvelteMap` named `alerts` and helper functions (`alert_error`, `alert_success`, `alert_warning`, `alert_info` with 2.5s auto-dismiss, `alert_appsync_error` with 5s auto-dismiss). Each alert has a UUID key, a close button, and a timeout-based auto-removal.

- **`auth.svelte.js`** — Cognito authentication state for admin users. Exports reactive `signed_user` object (`loading`, `id`, `email`, `groups`, `is_admin`). Provides `verify_user_signed_in()` (fetches session, extracts Cognito groups, checks "Admins" membership), `check_admin_and_redirect` (auth guard that redirects to Hosted UI or `/`), and `sign_out()`. Listens to Amplify Hub auth events and restores pre-auth path from sessionStorage after login.

- **`player.svelte.js`** — Player identity and registration. Exports reactive `player` object (`id`, `secret`, `name`) persisted to localStorage (key `yak_mania_player`). Exports `register_player(name)` (generates UUID secret, calls `registerNewPlayer` mutation), `update_player_name(new_name)` (calls `updatePlayerName` mutation), and `clear_player()`. Calls `get_appsync_client()` internally (imported from `game.svelte.js`).

- **`game.svelte.js`** — Core game state module (largest file). Exports reactive `game` object (`status`, `players` SvelteMap, `yak_counts`, `job_fees`) and lazy-initialized AppSync clients (`get_appsync_client` for API key, `get_appsync_admin_client` for Cognito userPool). Implements subscription buffering: high-frequency events write into non-reactive plain JS buffers (`Map`/variables), flushed into `$state` at 250ms intervals (~4Hz). A self-managing `setInterval` starts on first subscription event and auto-stops when there are no pending changes. Manages two subscriptions (`gameUpdated` and `removedPlayer`). `game_status` changes and resets bypass the buffer and apply immediately. Each player object is individually wrapped with `$state(p)` before being placed into the SvelteMap. Exports `load_game_state()`, `subscribe_game_updates()`, and `unsubscribe_all()`.

- **`validator.svelte.js`** — `ValidatedValue` class for reactive form validation. Takes an array of rule functions (return `true` or error string). Exposes `value`, `error`, `in_error`, `is_empty`, `display_error` properties. Has a 1-second debounce before showing errors, reset on each keystroke. `display_error_now()` forces immediate display on focus-out.

#### `lib/components/` — Svelte UI Components

- **`AlertDisplay.svelte`** — Renders global alert toasts. Fixed-positioned at the top of the viewport (`z-50`), iterates over the `alerts` SvelteMap. Each alert shows an icon, bold title, message, and close button. Must never overlap the game area.

- **`CModal.svelte`** — Generic reusable modal using DaisyUI's checkbox-based pattern. Accepts a `$bindable()` `open` prop and renders `children` inside a `modal-box`. No external dependencies.

- **`PlayerHeader.svelte`** — Top bar for the player game view. Shows "YM" branding, current player name with edit button, rank placeholder (`—`), and balance. If not registered, shows a "Tap to join" button. Integrates `CModal` for registration and name-editing forms, uses `ValidatedValue` for name input (3-30 chars).

- **`JobSelector.svelte`** — Bottom dock (DaisyUI `dock`) with three job buttons (Breeder, Driver, Shearer). Each displays the current job fee from `game.job_fees`. Buttons disabled when game status is `RESET` or player not registered. Exposes a `$bindable()` `selected_job` prop for two-way binding with the parent page.

- **`LoadingOverlay.svelte`** — Generic full-area overlay (`absolute inset-0 z-10`, semi-transparent `bg-base-100/70`) used during buy/sell transitions. Accepts `sprite` (image URL), `direction` (`'in'` for zoom-in, `'out'` for zoom-out), optional `waiting` flag, and `onfinished` callback. Plays a CSS `yak-zoom-in` or `yak-zoom-out` animation (1.5s); when finished fires `onfinished`. If `waiting` is true and the animation is done, shows a DaisyUI spinner with "Please wait…".

- **`DriverOverlay.svelte`** — Specialized truck animation overlay for the Driver job buy/sell transitions. Same full-area overlay pattern as `LoadingOverlay`. Accepts `direction` (`'in'` for buy, `'out'` for sell), optional `waiting`, and `onfinished`. Uses a 3-phase animation: (1) truck enters from one side (empty or loaded depending on direction), (2) brief pause, (3) truck exits to the opposite side with swapped sprite. Each crossing takes `CROSS_DURATION` (1s) with a `PAUSE_DURATION` (200ms) pause between. Includes a safety fallback timeout in case `animationend` doesn't fire. Shows a spinner when `waiting` and animation is done.

- **`BreederView.svelte`** — Breeder mini-game. Receives `yak` and `oncomplete` props. The player must tap 10 randomly-positioned haystack sprites before they disappear (1s timeout each). A progress counter (`🌾 collected / 10`) and truncated yak ID are shown at the top. The yak sprite evolves through three stages as haystacks are collected: `baby-yak-sprite` (0–4), `young-yak-sprite` (5–9), `hairy-yak-sprite` (10). The yak image size transitions accordingly (`h-1/6` → `h-1/4` → `h-1/3`). Haystacks spawn at random positions avoiding the center zone (35%–65%) where the yak sits. On completion (10 collected), the yak reverts to full size briefly, then becomes invisible and `oncomplete` fires after 600ms.

- **`DriverView.svelte`** / **`ShearerView.svelte`** — Minimal placeholder job views. Receive `yak` and `oncomplete` props. Display an emoji, "Driving/Shearing yak…" text, truncated yak ID, and an indeterminate DaisyUI progress bar. Auto-complete via `setTimeout` (1.5s) calling `oncomplete`.

#### `routes/` — SvelteKit Pages and Layouts

- **`+layout.js`** — Exports `prerender = true` for full SSG. Do not modify.

- **`+layout.svelte`** — Root layout. Imports `app.css`, mounts `AlertDisplay` globally (before route content), renders child pages via `{@render children()}`. No wrapper elements — each route manages its own layout.

- **`+page.svelte`** — Main game page (`/`). Mobile-first fullscreen layout (`h-dvh flex flex-col`). On mount: initializes API key client, loads game state, and subscribes to updates. On destroy: unsubscribes all. Renders `PlayerHeader` (top), conditionally shows job views or a "Select a job" prompt in the middle area, and `JobSelector` (bottom). `selected_job` is `$state(null)`, bound bidirectionally to `JobSelector`.

- **`admin/+layout.svelte`** — Auth guard for all `/admin/*` routes. Uses `$effect(check_admin_and_redirect)` to redirect unauthenticated users to the Cognito Hosted UI, and non-admin users to `/`. Shows a loading spinner while checking auth, then renders children only if the user is a confirmed admin.

- **`admin/+page.svelte`** — Admin control panel (`/admin`). Desktop layout (`max-w-4xl mx-auto`). Provides Start/Stop/Reset buttons (state-dependent enabling, `in_operation` guard), a player list table with per-row Remove buttons (uses `SvelteSet` named `removing` for concurrent per-player operation tracking), game status display, link to dashboard, and sign-out. Uses `get_appsync_admin_client()` for privileged mutations. `sorted_players` is a local `$derived` sorting alphabetically by name.

- **`admin/dashboard/+page.svelte`** — Live dashboard (`/admin/dashboard`). Designed for a 4:3 monitor with no scrolling. Displays total yak count prominently, a 7-column grid of yak location counts (Nursery, Breeding, Warehouse, Driving, Shearing Shed, Shearing, Sheared), and a sorted player leaderboard with rank, name, balance, bred/driven/sheared counts. Uses inline `$derived.by()` for ranking (by balance descending, ties broken alphabetically, same balance = same rank). All subscriptions stay active at all times.
