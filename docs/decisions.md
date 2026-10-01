# Decisions

Every decision with lasting impact, newest first. Each entry says what was decided, why, and what else was
considered. When experience contradicts a decision, add a dated amendment instead of rewriting history.

## 2026-10-01: Ten color schemes ship as catalog data

**Decision.** Croncave offers ten color schemes (Croncave, Ember, Harbor, Fern, Plum, Rose, Lagoon, Saffron,
Graphite, High contrast), each with light and dark values for every token. They live in
`crates/server/catalog/scheme-<id>.json`, listed in order by `schemes.json`, and people pick one in Settings ›
Appearance. Croncave stays the default. The build is described in `docs/color-schemes.md`.

**Why.** The founder asked for a choice of schemes, and the token system was built for this: components only use
`var(--token)`, so a scheme is new data, not new code. As files, every scheme goes through the same
`schemeProblems()` check, so none can ship with unreadable text or statuses that look alike. Each was designed on
the canvas first (a picker there recolors every screen), so the values were seen on real screens before they were
written down.

**Alternatives.** Generate schemes at runtime from one accent color (less control; results are hard to check
before people see them). Let people pick any accent color (a later step; most picks would fail the contrast checks
without help). Keep schemes as CSS files in the web app (schemes are platform data the control plane serves and
versions, like the plan catalog).

## 2026-10-01: Scripts can keep to part of the day, wait for files to settle, and report progress

**Decision.** The Scripts design's controls each have a backend:
- **Hours it runs.** A job can have a window (`window_start`/`window_end`, minutes of the day in the job's time
  zone; an end before the start wraps past midnight). The scheduler skips slots outside it, so "every 3 hours,
  from 6 AM to midnight" never runs at 3 AM.
- **Wait for more files.** A file-triggered job can wait `settle_secs` after the last change before it runs; each
  new change restarts the wait, so dropping in twenty files makes one run, not twenty.
- **Tell me when it finishes**, per run (`runs.tell_me`), on top of the job's own notification settings.
- **Progress.** The agent reads lines like `step 6,200 of 10,000`, `[31/212]`, `page 2 of 4` or `62%` from a run's output and reports
  done/total; the run page turns that into a bar and a time left. Scripts that print nothing get no bar.
- **Runtimes.** The agent reports the Python, Node.js and shell versions it has, so the Add screen offers what
  the computer can actually run.
- **Test before saving.** A new script is saved as a draft with a test run; Save turns it on.

**Why.** The design shows each of these; the founder asked for behavior behind every control.

**Alternatives.** Express windows in cron (users can't write it, and it can't wrap midnight cleanly). Debounce
file events on the computer (it may be asleep when files arrive through the control plane). Ask scripts to call
an SDK for progress (most scripts are someone else's; reading their output works without changes).

## 2026-10-01: Files' menu actions are real: zips, copies between computers, pins and Recent

**Decision.** The Files design's right-click menu and sidebar work end to end:
- **Folders download as a zip** made on the computer (in its system area, removed after sending).
- **Copy to another computer** streams the file, or the folder as a zip, through the control plane in 1 MB
  chunks into the other computer's resumable upload, which unpacks a zip into place. Both computers wake for it.
  Anything already at the destination goes to that computer's Trash, never overwritten.
- **Pins are a per-person preference** (`prefs.pins`, by computer), not a property of the folder.
- **Recent** is the newest files anywhere in the root, found by the agent (it stops looking after 50,000
  entries so a huge disk can't stall it).
- **Folders show how many things they hold**, and Trash shows days left (30, the agent's
  `CRONCAVE_TRASH_DAYS`).
- **"Open in Data" is shown but off**, because Data is coming soon.

**Why.** The design shows each of these; the founder asked for behavior behind every control.

**Alternatives.** Copy computer to computer directly (computers never listen, so the control plane relays it
anyway). Copy by re-uploading from the browser (slow, and the browser would need the file). Store pins on the
folder (they'd follow the folder to every person on a shared computer).

## 2026-10-01: Schedules run in the computer's time zone, chosen at sign-up

**Decision.** The sign-up's "Your details and phone" step asks for a time zone from the US zones (Eastern
through Chamorro, `crates/server/src/zones.rs`). A new computer takes its owner's zone; each job carries its
computer's zone, and the scheduler reads cron hours in it (`chrono-tz`), so "every day at 9 AM" means 9 AM
there, through daylight-saving changes. Schedules are described with the zone ("Every day at 9:00 am ET").
Existing rows default to Eastern (`0002_time_zones.sql`); before this, schedules were UTC.

Smaller calls in the sign-up, all from the Sign in and Plans and Billing canvases:
- **Sign in and Create account are separate pages** (`/signin`, `/join`) over the same one-time email link.
- **The link expires in 15 minutes**, as the canvas says, not 30.
- **The phone code is six boxes** that check themselves when the last digit lands; pasting fills them all.
- **The paid-plan checkout is a dialog over the plans** (the canvases have no checkout screen; payment is the
  mock provider's card form, as before).
- **Plan cards take a tagline and a "recommended" flag from the catalog** (`tagline`, `recommended`), and
  their checklists are worded from the catalog's numbers, so the cards stay true when the catalog changes.
- **First run is a page of its own** (`/welcome`, "Set up your first computer"), with the catalog's sizes as the
  starting points. Picking one creates "My computer" at that size; sizes above the plan's largest say which
  plan they need; "Start from scratch" opens the full form.

**Why.** The designs ask for the time zone at sign-up and show "Used for schedules like every day at 9 AM" in a
computer's settings; schedules in UTC would surprise everyone outside it. Carrying the zone on the job keeps
the scheduler's due-job query a single table scan.

**Alternatives.** A zone per person only (computers can live in other zones, and Settings › This computer has
its own picker). Store cron in UTC and convert at save time (wrong half the year, across daylight saving).
Every IANA zone (Croncave is US only; the list is the eleven the design shows).

## 2026-10-01: The web app follows the design canvases screen by screen

**Decision** (the founder's calls, asked before building):
- **Scope.** Every screen in the design canvases (`docs/design.md`) is rebuilt to match. Where a screen shows
  something the backend can't do yet (Watcher conditions and quiet hours, run progress with CPU and memory, the
  Files right-click menu, storage choices, sleep timing per computer), the backend is built too, so nothing on
  screen is fake. The work lands area by area, each pushed to `main`.
- **Sizes stay Small, Medium and Large** from the plan catalog. The New computer design's Light, Standard, Power
  and Heavy cards are drawn with the catalog's names and specs, because every price and limit comes from the
  catalog.
- **Apps and settings pages that aren't built are shown as "Coming soon"**: Data and Fetcher sit in the dock,
  dimmed and not openable, so the layout matches the design and people can see what's coming.
- **The Ask button shows for everyone, and can be put away.** With AI off it explains the assistant and offers to
  turn it on; nothing reaches an AI model until someone does. Putting it away is a per-person preference
  (`prefs.ask_hidden`), undone in Settings › Assistant.

My calls while building the shell:
- **The design's extra colors become tokens** (scheme version 2): `raised` (tiles), `track` (meter tracks),
  `line-strong` (field borders), `accent-hover` (link hover), a soft fill per status (`live-soft` … `asleep-soft`)
  for the status pills, four code-highlighting colors, and `shadow`. Light values come from the canvases' light
  screens where they exist; the rest are chosen to keep the scheme's contrast checks passing.
- **The top bar shows usage left, not spend against the cap.** The canvases have both; the Plans and Billing
  canvas, the newer one, replaced "This month $6.20 / $25" with "Plus · usage left $1.60" and a meter. Usage left
  matches how the product meters: free usage first, then overage only if turned on.
- **Statuses are pills by default** (`<Status>`), a word on a soft fill of its color, as in every canvas; dense
  rows can ask for the plain form.
- **Icons are the canvases' own stroke icons**, collected into `web/src/lib/ui/icons.ts`, instead of an icon
  library, so the app draws exactly what the designs draw.

**Alternatives.** Restyle the existing pages without changing their structure: faster, but the designs change
layouts (framed app windows with a list pane, the sign-up as separate steps), not just colors. Adopt the
design's four sizes: rejected by the founder in favor of the catalog. Hide unbuilt apps until they exist: the
founder preferred showing them as coming soon.

## 2026-10-01: pnpm is pinned per package, and the lockfiles satisfy the release-age policy

**Decision.** `web/package.json` and `e2e/package.json` carry `"packageManager": "pnpm@12.6.0"`, and both
lockfiles are resolved under pnpm 12's default `minimumReleaseAge` (a package must be a day old). Where a
dependency's floor demanded a version younger than that — `vitest` was pinned `^5.0.3`, and 5.0.3 was hours old —
the floor is lowered (`^5.0.0`, which resolves 5.0.2) rather than excluded from the policy. The root
`package.json` carries the same pin, and CI's `pnpm/action-setup` reads it instead of naming a version, so the
workflow holds no version of its own. The three pins must match when pnpm is upgraded.

**Why.** The lockfiles were resolved with pnpm 10.28, which had no release-age policy, so `pnpm install
--frozen-lockfile` failed outright on a machine with pnpm 12 (`ERR_PNPM_MINIMUM_RELEASE_AGE_VIOLATION`). The policy
is worth keeping: a day's delay is what catches a compromised release before it reaches the repo. Pinning the
version in each manifest means every machine and CI resolve the same way, so a lockfile committed on one machine
installs on the others.

**Alternatives.** Turn the policy off (`minimumReleaseAge=0`), which gives up the protection that made the install
fail for a good reason. Let pnpm write `minimumReleaseAgeExclude` for the packages it couldn't satisfy — it offers
to, and it is the same thing as turning the policy off, one package at a time, with no record of why. Commit the
lockfile that bypassed the policy locally (the error message names this as the case to distrust).

## 2026-10-01: Docker computers boot a computer image and reach the relay over the bridge

**Decision.** `COMPUTE_DRIVER=docker` (or `./scripts/dev.sh --docker`) runs each computer as a container from the
computer image, `docker/computer.Dockerfile`: Debian with Python, Node.js, bash and `ps`, and the agent as its only
service, behind `--init` so orphaned processes are reaped. The disk is a bind mount; a container is made at each wake
and removed at sleep. CPU and memory limits come from the computer's size. Nothing is published: the agent dials
`host.docker.internal`. On Linux that is the bridge gateway, so the control plane gets an extra listener there
(`RELAY_ADDR`) that serves only the relay and the demo sites, never the API. Docker Desktop (macOS, Windows) already
routes `host.docker.internal` to the host. Where computers reach the demo sites is configuration (`DEMO_URL`), and
the built-in demo types (version 2) take their default address from it.

The image has two ways to get the agent: built inside Docker (works from a Mac; a CA bundle can be passed as a build
secret for TLS-inspecting proxies), or copied from a Linux host's build (seconds instead of minutes).
`scripts/build-computer-image.sh` picks one. The shared driver suite and all 13 browser flows pass on Docker
(`E2E_DOCKER=1`); wakes took 340 ms (median) to 463 ms.

**Why.** The first Docker driver mounted the host's agent binary into a stock Python image. That can't work on a Mac
(a macOS binary in a Linux container) and depends on matching C libraries on Linux. It also couldn't reach a control
plane listening on 127.0.0.1, or the demo sites.

**Alternatives.** Listen on 0.0.0.0 (exposes the whole API to the local network). `--network host` (computers would
share the host's network again, losing the isolation that is the point of Docker). A static musl agent (rustls'
crypto needs a C toolchain for musl; more build trouble than it saves).

**Not yet.** Disk size limits (a bind mount has none), egress filtering, and running the agent as a non-root user
inside the container.

## 2026-10-01: dev.sh watches for changes; a `git pull` is the whole loop

**Decision.** While `./scripts/dev.sh` runs, Rust changes rebuild and restart the control plane (computers keep
running and reconnect; they get a rebuilt agent at their next wake, as the architecture's agent updates do). A failed
build leaves the previous version running. A changed web lockfile reinstalls packages; Vite already reloads the web
app. `--no-watch` turns it off; `--e2e` never watches. In-memory dev state (the moved clock, planted demo data)
resets on a restart.

**Why.** The founder iterates by pulling the branch and trying it. Without this, every pull meant stopping and
restarting the script.

**Alternatives.** cargo-watch or watchexec (another tool to install). A public preview link from the agent's cloud
session (the session's container takes no incoming connections; a tunnel would make the app and its Dev tools,
which show sign-in links, public).

## 2026-10-01: Light-mode "needs you" is #9a5b00, not #c2410c

**Decision.** The scheme check (text 4.5:1, controls 3:1, statuses distinguishable) runs as a unit test on every
scheme. It found the design's light "needs you" orange (#c2410c) only ΔE 16 from "failed" red (#b91c1c), too close
to tell apart at a glance. Light "needs you" is now a deep amber, #9a5b00: ΔE 41 from "failed", and 4.8:1 or better on
every light surface. Dark mode is unchanged. `docs/design.md` notes the change.

**Why.** The architecture requires status colors that are "distinguishable from each other". Statuses always carry a
word too, but "needs you" and "failed" are the two a person must never confuse.

**Alternatives.** Keep the orange and rely on the word (fails the scheme's own check). Change "failed" instead (red
is the stronger convention). Measure distance in RGB (doesn't match perception; ΔE in CIE Lab does).

## 2026-10-01: Only screens that read the computer wake it

**Decision.** Files, Code and previews read the computer itself, so opening them wakes it and keeps it awake while
they're visible (the page reports presence every 15 seconds; presence lapses 40 seconds after the last report).
Home, Activity, Scripts and Watcher show results the control plane already stored, so opening them lets a sleeping
computer sleep.

**Why.** "A computer is awake only while work runs or someone is looking." Looking at last night's watch results
doesn't need the computer, and waking it would cost money for nothing.

**Alternatives.** Wake on opening any app (what the architecture literally lists; wasteful for result screens).
Never wake for viewing (Files couldn't work).

## 2026-10-01: The web app is a client-rendered SvelteKit app; the theme is applied by the server hook

**Decision.** SvelteKit runs with `ssr = false`: the server serves the shell, and the browser talks only to `/api`
(proxied to the control plane in development, the edge's job in production). The server hook injects the scheme's
CSS custom properties and the person's mode (from a cookie) into the HTML, so the first paint is already themed.
Schemes come from the control plane as data, with the bundled scheme as a fallback.

**Why.** Every screen is behind sign-in and live, so server rendering adds cookie forwarding and double data loading
for no gain, while the theme still has to be right before first paint.

**Alternatives.** Full SSR (more moving parts). A static SPA with an inline script reading localStorage (works, but the
scheme would have to be duplicated into the HTML template).

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
