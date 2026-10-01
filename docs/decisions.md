# Decisions

Every decision with lasting impact, newest first. Each entry says what was decided, why, and what else was
considered. When experience contradicts a decision, add a dated amendment instead of rewriting history.

## 2026-10-01: Run the relay and the preview edge inside the control plane process (prototype)

**Decision.** One `croncave-server` binary serves the API, the relay (`/relay/*`) and the preview edge (its own port).
The relay is its own crate (`croncave-relay`) and talks to the control plane only through a `RelayHooks` trait and a
`Relay` handle, so it can move to its own process without changing either side.

**Why.** "One command" must stay simple, and an internal RPC between relay and control plane adds a moving part
that proves nothing about feasibility. The hard parts (the agent dialing out, stream multiplexing, reconnects,
exactly-once events) are all real.

**Alternatives.** Separate relay process now (needs a relay-to-control-plane protocol and a routing table service
before there is a second relay node). Relay as a module of the server crate (no enforced boundary).

## 2026-10-01: The wire protocol: binary frames over one WebSocket, JSON control on stream 0

**Decision.** Every WebSocket message is `[kind u8][stream u32][payload]`. Stream 0 carries JSON control messages
both ways. Only the relay opens streams (odd ids); a stream is a two-way byte pipe with half-close (`End`) and
`Reset`. Files, previews and code review each open a typed stream. Backpressure comes from bounded channels.

**Why.** A typed open plus raw bytes keeps a 2 GB download from blocking a control message, and needs no extra
dependency (yamux, HTTP/2) inside the agent. Agent-initiated work (SDK calls like market quotes) goes over control
with request ids, so the agent never needs to open streams.

**Alternatives.** yamux or HTTP/2 inside the WebSocket (more machinery, harder to debug). One WebSocket per stream
(more connections through proxies, breaks "one connection"). Not yet done: per-stream flow-control windows, and
WebSocket upgrades through previews (live reload falls back to manual refresh).

## 2026-10-01: Agent events are durable and stored exactly once (epoch + sequence)

**Decision.** The agent appends every event to `system/agent/outbox.jsonl` with a sequence number and an epoch (a
random id kept with the disk). The relay acknowledges a batch only after the control plane stored it in the same
transaction that advances the computer's `agent_acked_seq`. Resends after a reconnect are skipped by sequence; a reset
disk starts a new epoch.

**Why.** "Logs and events are buffered on disk until the relay acknowledges them" plus "each scheduled time runs
once" means results must not be lost or doubled. Run output is in the same stream, so live output survives a dropped
connection too.

**Alternatives.** At-most-once (simpler, loses results). Idempotency keys per event in a separate table (more rows,
same effect).

## 2026-10-01: Per-run snapshots with hard links instead of a copy-on-write filesystem (local driver)

**Decision.** Before each script run the agent makes a tree of hard links of the person's root. After the run it
compares the trees (size and modification time) to find created, changed and deleted files, and moves anything the
run deleted into Trash from the snapshot, attributed to the run. The Code app keeps a real copy of the project
instead, because review needs the old contents of files the agent edits in place.

**Why.** Hard links are instant and portable (Linux and macOS, no special filesystem), and a deleted file stays alive
through its link. That gives "Trash catches every delete" and "what this run made" without btrfs or ZFS.

**Alternatives.** btrfs/ZFS snapshots (the production plan; not available on a laptop). A full copy per run (slow
for large folders). Watching the filesystem (misses changes while disconnected, platform-specific).

## 2026-10-01: Computers in the local driver are supervised processes; Docker is opt-in

**Decision.** `COMPUTE_DRIVER=local` runs each computer as an agent process in its own process group with its own
disk folder under `DATA_DIR`, with a pid file so a restarted control plane can still see and stop it.
`COMPUTE_DRIVER=docker` runs the same agent in a container (CPU and memory limits, its own network namespace). The
shared driver test suite runs against `local` always and against `docker` when a daemon is up. `fly` returns "not
configured".

**Why.** No Docker daemon is assumed on the founder's laptop (or was available where this was built), and the local
driver is enough to prove sleep, wake, the outgoing connection and disk persistence.

**Known limits of `local`.** Computers share the host's network: a dev server on port 5173 in one computer would be
reachable from another, and two computers can't both use 5173. Previews still only travel over the agent's
connection, but the isolation the architecture requires comes from microVMs (or at least the Docker driver). The
Docker driver mounts the host-built agent binary, so the image must have a compatible libc; it wasn't exercised here
because no daemon was available.

**Alternatives.** Docker only (one more thing to install; slower wakes). Firecracker locally (Linux-only, needs KVM).

## 2026-10-01: Business time can be moved forward in development; security and billing durations can't

**Decision.** Everything that is business time (trials, billing months, schedules, "since you left") reads an
adjustable clock that Dev tools can move forward (`CLOCK=adjustable`). Expiries that protect people (sessions,
sign-in links, phone codes, preview and bootstrap tokens) use real time. Awake seconds and storage are measured with a
monotonic clock, so moving the clock never bills time that didn't pass.

**Why.** Trials ending and months rolling over must be demonstrable in a minute, not two weeks. Moving the clock
forward 32 days must not sign everyone out or bill a month of awake time.

**Alternatives.** Dev buttons per feature ("end trial now", "close month now"): quicker to build but each is a
special path that skips the real logic. A fake clock everywhere: breaks sessions and metering.

## 2026-10-01: What the spending cap counts, and the order buckets are drawn

**Decision.** Usage draws from the sign-up award, then credits (promos, support), then a trial's allowance, then the
monthly allowance, then overage (only if opted in). The cap counts what was drawn from the allowance and overage this
month; the award, credits and trial allowance are gifts that don't count toward it. The cap defaults to the monthly
allowance. When nothing more can be drawn, the rest is recorded as `unbilled` (never charged later) and work pauses.
Raising the cap, a credit, or a new month resumes held runs.

**Why.** The product doc says the cap "starts at the allowance" and that the award is spent first. If the award
counted toward the cap, a Free account (award $2, allowance $1) would pause with $1 of its award unspent.

**Alternatives.** Cap on all usage including the award (contradicts the doc's numbers). Credits drawn after the
allowance (makes a support credit invisible until the allowance runs out).

## 2026-10-01: Storage is billed by what is stored, not the disk's size

**Decision.** The storage meter bills GB actually used (files plus Trash, from the agent's health reports), at the
catalog's per-GB rate with markup, whether the computer is awake or asleep. The disk's size is a limit, not a charge.

**Why.** A Small computer's 10 GB disk at $0.15/GB-month plus 25% is $1.88 a month, more than the Free allowance
($1); billing the size would pause every Free account in a fortnight. The architecture's meter is "Disk GB per
computer, sampled hourly", which fits used GB.

**Alternatives.** Bill the provisioned size (matches some provider bills, breaks the Free plan). Include some free GB
per plan (a catalog field to add if used GB turns out too generous).

## 2026-10-01: Plans are paid in advance, overage in arrears, on one invoice a month

**Decision.** Choosing a paid plan charges the first month at once. At each month's end the ledger closes the period:
one invoice with next month's plan price and this month's overage, the unused allowance expires, the new allowance
is granted. Trials never charge; keeping the trial plan is a normal plan change paid at that moment. Changing plans
mid-month charges the new price from today and tops up the allowance by the difference (no proration of the old
plan yet).

**Why.** "Each customer is charged once" a month, and the mock provider records the same shape of charges Stripe
will.

**Alternatives.** Stripe subscriptions as the source of truth (ties plan rules to Stripe). Proration on plan change
(right eventually; noise in a prototype).

## 2026-10-01: Mock checkout takes card numbers; the real one won't

**Decision.** The `PaymentProvider` mock accepts Stripe's documented test numbers through our own form (4242… works,
4000…0002 is declined, 4000…9995 fails at charge time, and so on). The real Stripe implementation will take a
payment method id from Stripe's own card form instead, so card numbers never reach our servers; `CardInput` becomes
that id.

**Why.** It exercises declines and fingerprints (awards are tied to the card) without a Stripe account.

**Alternatives.** Skip cards in the prototype (can't test award-per-card or declines).

## 2026-10-01: Live updates over server-sent events fed by Postgres NOTIFY

**Decision.** Events, run status, output lines and computer state are announced with `pg_notify` inside the same
transaction that writes them; one listener per control plane fans them out to the web app over SSE (`/api/live`).
The web app refetches what changed.

**Why.** Notifications go out only on commit, so the UI never shows something that rolled back, and any control
plane instance can serve any browser. SSE passes through the dev proxy and reconnects by itself.

**Alternatives.** A WebSocket per browser (two-way isn't needed yet). Polling (slow and wasteful).

## 2026-10-01: Dev servers are found by probing the port they were given

**Decision.** The Code app starts a dev server with `PORT` set and the agent probes that port on localhost to report
"Running · open preview". A dev server alone doesn't keep a computer awake; an open preview or the Code app does.
Dev servers stop when the computer sleeps and are started again from the Code app.

**Why.** Probing works the same on Linux and macOS (scanning listening sockets needs `/proc` or `lsof`), and a dev
server nobody is looking at shouldn't cost awake time all night.

**Alternatives.** Scan `/proc/net/tcp` (Linux only). Keep the computer awake while any dev server runs (expensive).

## 2026-10-01: Previews on `*.preview.localhost` with a one-time token and a host-only cookie

**Decision.** Each preview gets a random subdomain of `PREVIEW_DOMAIN` (`p<16 hex>.preview.localhost:8081` locally).
The Code app mints a 60-second one-time token; the preview address trades it for a `SameSite=None; Secure;
HttpOnly` cookie scoped to that one host, and strips that cookie before forwarding to the person's server. Browsers
treat `*.localhost` as secure, so this works over plain http locally.

**Why.** Code people write runs on that origin, so it must share nothing with the app's origin, and a token in a
URL must not stay useful.

**Alternatives.** Path-based previews on the app's domain (shares cookies and storage: unsafe). A long-lived signed
URL (leaks through history and referrers).

## 2026-10-01: The mock coding agent is a separate process with an approval protocol

**Decision.** The Code app's agent runs as a child process (`croncave-agent mock-coder`), exactly where the real
Claude or OpenAI tool will run. It edits the project, then asks before running a command by printing an
`@@needs-approval` line; the supervisor turns that into a "Needs you" event, and the answer goes back on its stdin.
Before the task starts the worker copies the project, so review shows every file's before and after, and Keep or Undo
works file by file.

**Why.** It exercises long-running supervised processes, live output, approvals and review the same way the real
tools will, with no provider account.

**Alternatives.** Fake the agent inside the control plane (proves nothing about running on the computer). Use git
branches for review (needs git in every computer and a repository in every project).

## 2026-10-01: Smaller calls made while building the prototype

- **Runtime-checked SQL.** Queries use sqlx at runtime rather than its compile-time macros, so building needs no
  database. Every query is exercised by the database tests instead.
- **App manifests in code.** The four R1 apps are built in, so their manifests live in `apps.rs` (served at
  `/api/apps`) rather than a table; a table arrives with third-party apps.
- **Watcher types.** Blank inputs take the type's default. A threshold's direction is a template, so one type serves
  "above" and "below". A first "page changed" check saves a baseline; a first "new items" check reports what matches
  now. Test runs never save state or notify. Market data is an SDK call answered by the control plane, so a real
  provider's key never reaches a computer.
- **Notifications.** Email is on by default; text and push are opt-in. Repeats (three or more of one kind in a
  delivery pass) become one message. Quiet hours hold non-urgent messages. Push is stored in the outbox as its own
  channel until web push exists.
- **Packages inside computers.** Scripts install Python packages per requirements file into the app's own area and
  reuse them; Node scripts run `npm install` beside `package.json`, because that is what the person's project
  expects. pnpm remains the rule for our own web app.
- **US only.** Sign-up requires a US mobile number. An IP location check needs a provider and waits.
- **Not yet.** Zipped folder downloads, GitHub connections (so "open a pull request" is shown but disabled),
  assistant-drafted watcher types (the API to add and approve user types exists), per-computer AI switches, and
  daily alert limits.

## 2026-10-01: Build a locally runnable prototype first, with mocked providers

**Decision.** Before any paid account is set up, build a prototype that runs on one machine with one command, using
the decided stack, with every external provider (Stripe, Fly, email, SMS, push, AI, market data) behind its real
interface and a mock implementation chosen by configuration.

**Why.** The founder wants to verify the product is feasible before committing money or time to provider setup.
Mocks behind real interfaces mean the prototype becomes the start of the product rather than throwaway code.

**Alternatives.** Set up the real providers first (slower, spends money before feasibility is known). Build a
clickable design prototype only (proves the UI, not the hard parts: sleep and wake, the outgoing connection,
scheduling, billing).

## 2026-09-30: Stack

**Decision.** Web app in TypeScript with Svelte 5 and SvelteKit, built with pnpm. Control plane, relay, agent and AI
gateway in Rust (Tokio, axum, sqlx). Postgres for records, the run queue, events and the ledger. S3-compatible object
storage. Compute behind a driver interface.

**Why.** Rust lets the agent ship as one small static binary and gives predictable performance for the relay's
long-lived connections. Postgres covers records, queueing (row locks) and events without extra services. Svelte keeps
the web app small and fast. pnpm is strict about dependencies and fast on CI.

**Alternatives.** Go for the services (also a good fit; Rust chosen for the agent binary and the founder's
preference). Node.js for the services (simpler hiring, weaker fit for the agent). React for the web app (larger
ecosystem; Svelte chosen). A separate queue such as Redis or a workflow engine (an extra service before it's needed).

## 2026-09-30: Plans, awards and trials are data in a versioned plan catalog

**Decision.** Prices, limits, sign-up awards, monthly allowances, trials, promos, provider rates and markups live in a
versioned plan catalog stored as data. Accounts stay on the catalog version they signed up on. Usage draws from the
award, then the monthly allowance, then opted-in overage, recorded in one append-only ledger per account.

**Why.** Pricing is unproven and will change; changing it must not need a release, and existing users must be able to
keep old terms.

**Alternatives.** Prices in code or configuration files (needs a deploy to change; no history). Stripe as the source of
truth for plans (ties product rules to one payment provider and can't express awards and allowances cleanly).
