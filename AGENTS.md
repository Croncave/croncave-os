# Croncave: brief for coding agents

Read this file fully before doing anything, then read `docs/product.md` and `docs/architecture.md`.
`CLAUDE.md` points here, so this is the one brief every agent shares.

## What Croncave is

Croncave gives anyone a private computer in the cloud that keeps working while they're away, used through simple
apps in a browser: **Watcher** (watch pages, prices, listings), **Scripts** (run your own Python, Node.js or shell
scripts by hand, on a schedule or when files change), **Code** (edit a project and hand a coding agent a task),
**Files**, **Home** and **Activity**. Computers sleep when nothing is active and wake for work. AI is optional and off
by default. US only. Web only.

## The current goal: a locally working prototype

The founder wants to verify the product is feasible before setting up any paid accounts. Build a prototype that
runs entirely on one machine with **one command**, and that someone can click through end to end:

1. Sign up (email sign-in link, phone code), choose a plan, get the sign-up award, accept or skip the trial.
2. Create a computer (Small, Medium or Large), see it wake, go to sleep about 30 seconds after nothing is active,
   and wake again on demand or for a scheduled job.
3. Files: upload, browse, preview (table, image, text), delete to Trash and restore, see which run made each file.
4. Scripts: add a script, run it now and on a schedule, watch live output, see the result (summary, changed files).
5. Watcher: set up a watch from a type (a web page, a demo listings site, a demo stock price), test it, see history
   and alerts.
6. Home and Activity: what happened since you left, unread notifications, live updates without refreshing.
7. Plans and billing: usage drawn from the award, then the monthly allowance, then overage if turned on; the cap
   pausing work; trials ending with no charge; the screens in the Plans and Billing design.
8. Code: open a project, a (mock) agent makes changes you review, a dev server runs and opens in a private preview.
9. Admin: edit the plan catalog, give an account a credit, see the measurements dashboard.

### Mock anything that needs the founder's accounts

Stripe, Fly, email, SMS, push, Claude and OpenAI keys, market data and domains are **mocked behind the same
interfaces the real providers will use**, so swapping in the real one is a new implementation, not a rewrite:

- **Compute:** a `ComputeDriver` trait with a `local` driver (each computer is a supervised process with its own disk
  folder; use Docker containers instead when a Docker daemon is available) and a `fly` driver stub that returns
  "not configured".
- **Payments:** a `PaymentProvider` with a `mock` implementation (a fake checkout that accepts the documented Stripe
  test card numbers, and records charges and invoices in the database).
- **Email, SMS, push:** a `Notifier` with an `outbox` implementation that stores messages; a dev page shows the
  outbox, so sign-in links and phone codes are readable there.
- **AI:** an `AiGateway` with a `mock` model that returns canned but plausible answers and meters fake tokens; the
  Code app's agent is a scripted mock that edits files in the project.
- **Market data:** a mock stock price source; the Watcher's demo pages are served locally.

Every mock is chosen by configuration (`.env`), never by `if dev` branches scattered through the code.

## Stack (decided; see `docs/decisions.md`)

| Part | Technology |
| --- | --- |
| Web app | TypeScript, Svelte 5 and SvelteKit, built with **pnpm** (never npm) |
| Control plane, relay, agent, AI gateway | Rust (stable, edition 2024): Tokio, axum, sqlx, serde |
| Database | Postgres (records, run queue, events, ledger) |
| Object storage | S3-compatible (MinIO locally) when needed; local disk is acceptable in the prototype |
| Agent transport | One outgoing WebSocket from the agent to the relay, with a stream multiplexer |

Python is **not** part of the platform. It exists only inside users' computers, as one of the runtimes the Scripts app
offers.

## Hard rules

- **Computers never listen.** The agent only dials out to the relay. No code path may expose a port or address on a
  computer. Previews reach a computer's localhost through the relay.
- **Provider-neutral outside the driver.** Nothing outside the compute driver knows which provider runs computers.
- **Apps are written by hand** on the platform SDK (jobs and runs, files, app data, events, secrets, AI, previews,
  settings). Watcher's types are config read by the watcher engine; that is Watcher's choice, not how apps are built.
- **Every price, limit, award, trial and promo comes from the plan catalog** (data, versioned). Nothing hard-codes a price.
- **Colors come from semantic tokens only.** No hex values in components; schemes are data with light and dark values.
- **Secrets never appear in the repo, logs or fixtures.** Use `.env` (with `.env.example` listing every variable).
- **AI is opt-in.** Every feature works with AI off.

## How to work

- **Decide, then document.** When a decision is needed, make the best call yourself and record it at the top of
  `docs/decisions.md`: the decision, why, and the alternatives considered with why each was rejected. Don't stop to ask.
- **Small, reviewable commits** with an imperative summary and a body explaining why. Keep the checks green.
- **Tests:** unit tests beside the code; database tests against a real Postgres; end-to-end tests with Playwright in
  `e2e/`. A test that can't fail isn't a test.
- **One command to run everything locally** (`./scripts/dev.sh` or `pnpm dev` at the root): starts Postgres, the
  control plane, the relay, the web app, and computers through the local driver. **One command to check everything**
  (`./scripts/check.sh`), which CI also calls.
- **Plain words in the UI.** "Asleep", "awake", "needs you". No "instance", "vCPU" or "container" in the product.
- **Measure from day one.** Wake time, awake time by cause, usage per account, plan funnel events (see the
  Observability section of `docs/architecture.md`).

## Where things are

| Path | What |
| --- | --- |
| `docs/product.md` | Press release, FAQ, every story with priority and release, pricing |
| `docs/architecture.md` | How it is built, and the build order |
| `docs/decisions.md` | Every decision with lasting impact, newest first |
| `docs/design.md` | Links to the design canvases and the design tokens |
| `docs/local-guide.md` | Starting the prototype locally and trying each flow |
| `crates/proto` | The agent–relay wire protocol and shared types (run specs, watcher types) |
| `crates/relay` | The relay: agent connections, credentials, stream multiplexing |
| `crates/agent` | The agent on every computer: Files, runs, the watcher engine, the Code worker, previews |
| `crates/server` | The control plane: API, orchestrator, scheduler, ledger, providers (mocks), preview edge |
| `crates/server/catalog` | Data: the plan catalog seed, the color scheme, built-in watcher types |
| `web` | The SvelteKit web app |
| `e2e` | Playwright tests, one per prototype flow |
| `scripts` | `dev.sh` (run everything), `check.sh` (check everything) |
