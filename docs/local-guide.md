# Running Croncave locally

Everything runs on one machine with one command. Payments, email, text messages, AI, market data and the cloud
provider are mocks behind their real interfaces, so no accounts are needed.

## What you need

- **Rust** (stable, 1.88 or newer): `curl https://sh.rustup.rs -sSf | sh`
- **Node.js 22 or newer** and **pnpm**: `corepack enable` picks up the pinned version
  (`packageManager` in `package.json`), so you don't choose one.
- **Postgres 16 or newer** binaries (`brew install postgresql@17` on a Mac, `apt install postgresql-16` on
  Ubuntu), or a running Docker. You don't start Postgres yourself; the script runs a private one in
  `.dev/postgres`.
- **Python 3** (scripts and the demo dev server run with it) and optionally Node.js for Node scripts.

## Start it

```sh
./scripts/dev.sh        # or: pnpm dev
```

The first run builds the Rust services and installs the web app's packages (a few minutes). Then open
**http://localhost:5173**. Ctrl-C stops everything, computers included.

Leave it running: when you `git pull` (or edit), the web app reloads in the browser and Rust changes rebuild and
restart the control plane by themselves. Computers keep running through a restart. `--no-watch` turns this off.

It creates `.env` from `.env.example` with fresh secrets. Every provider is chosen there (`COMPUTE_DRIVER=local`,
`PAYMENTS=mock`, `NOTIFIER=outbox`, `AI=mock`, `MARKET_DATA=mock`). Ports: web 5173, API and relay 8080, previews 8081.

**Dev tools** (http://localhost:5173/dev, also in the account menu) is where you read the simulated email and text
messages, move time forward, plant demo data and add usage.

## Try each flow

1. **Sign up.** Enter any email on the sign-in page. On Dev tools, click **Open the sign-in link**. Enter any US
   mobile number (e.g. `(415) 555-0123`); the code is on Dev tools. Pick a plan. For a paid plan use the test card
   `4242 4242 4242 4242` (any future expiry, any CVC); `4000 0000 0000 0002` is declined. You get the sign-up award;
   accept or skip the trial. To see the admin pages, sign up as `admin@croncave.local`.

2. **Computers.** Create one (Small, Medium or Large; your plan decides which). The top bar shows it waking, then
   **Awake** (usually in well under a second locally). Stay on **Home** (which doesn't need the computer) and it goes
   **Asleep** about 30 seconds later. Open **Files** and it wakes again. **Settings** (bottom of the dock) has sleep
   delay, keep-awake, size, restart, reset, delete, and under *Details* the measured wake times.

3. **Files.** Upload (button or drag and drop; large files go in resumable 1 MB chunks), make folders, click a CSV,
   image or text file for a preview made on the computer. Delete moves to **Trash**; restore from there. The *From*
   column says who made each file, e.g. *Made by "Sales summary"*.

4. **Scripts.** **Add a script** → *Summarize a spreadsheet (Python)* → **Save and run now**. Watch the output live,
   then the result: headline, values and the file it made. Try *On a schedule* or *When files change* (e.g. folder
   `Inbox`, then upload into `Inbox`). A script that imports a missing package fails with the reason and the fix.

5. **Watcher.** **New watch** → *Apartment listings (demo site)*, read the plain-words rule, **Test it now** (nothing
   is saved), then **Save and start watching**. On Dev tools click **Add a listing**, then **Check now**: a new match
   and an unread notification. *A stock price (demo data)* draws a chart; use **ACME −10%** on Dev tools to push it
   across your price. *A web page* watches the bakery page; **Change the bakery page** to see a change.

6. **Home and Activity.** Home shows what happened since you left, what needs you (approvals, failures with why),
   latest results and what's next. The bell opens **Activity** (All / Unread, mark all read). Everything updates
   live; there's no refresh button. Notifications also go to the outbox by email (text and push if you turn them on
   in **Settings**, where quiet hours live too).

7. **Plans and billing.** The top bar shows usage left. **Plans and billing** shows the award, allowance, trial
   allowance and credits, the cap with alerts at 50/80/100%, plans, invoices and promo codes (try `ALPHA5`). On Dev
   tools, **+$2 usage** a few times: usage draws from the award, then the allowance; on Free, work pauses (*Usage ran
   out*, computers sleep, nothing is deleted). Switch to Plus, turn on overage and raise the cap: work resumes. To
   end a trial, sign up with a trial and press **+15 days**; to close a month and get an invoice, **+31 days**.

8. **Code.** Make a project (it starts as a tiny website), open a file in the editor, edit and save (Ctrl/Cmd-S).
   In the **Agent** tab hand it a task such as *Make the heading say "Fresh bread daily"*. It asks before running a
   command (*Needs you*, also on Home): approve. Then **Review changes**: keep or undo each file. In **Preview**,
   start the dev server and **Open preview**: your page on its own private address (`*.preview.localhost:8081`), at
   laptop or phone width.

9. **Admin** (signed in as `admin@croncave.local`, account menu → Admin). **Plan catalog**: change a price, award or
   limit and save a new version (new sign-ups get it; existing accounts keep theirs). **Accounts**: give a credit with
   a reason. **Measurements**: wake times, why computers were awake, the plan funnel, activation and usage.

The assistant is off by default: turn it on in **Settings**, then **✦ Ask** in the top bar, e.g. *tell me when ACME
goes below $90*, and apply its proposal.

## Check everything

```sh
./scripts/check.sh                    # what CI runs
CHECK_SKIP_E2E=1 ./scripts/check.sh   # without the browser tests
```

The browser tests need Playwright's Chromium once: `cd e2e && pnpm install && pnpm exec playwright install chromium`.
They start their own stack (`./scripts/dev.sh --e2e`, ports 15173/18080/18081, a fresh database).

## Good to know

- Computers in the local driver are processes in `.dev/data/computers/<id>/`; `disk/root` is the person's files.
  They share your machine's network, so give dev servers distinct ports (see `docs/decisions.md`).
- To start over: stop the script, then `rm -rf .dev` (this deletes the local database too).
- `fly` (as `COMPUTE_DRIVER`) says it isn't configured until the real implementation exists.

## Computers as Docker containers

```sh
./scripts/dev.sh --docker
```

With Docker running, this builds the computer image (`docker/computer.Dockerfile`; on a Mac the first build compiles
the agent inside Docker and takes a few minutes) and runs each computer as a container: its own network, CPU and
memory limits from its size, its disk mounted from `.dev/data/computers/<id>/disk`. Everything else works the same,
and dev servers no longer share your machine's ports. `docker ps --filter label=croncave.computer` shows the awake
ones. To run the browser tests this way: `cd e2e && E2E_DOCKER=1 pnpm test`.

If your `.env` was made before this, it has an unused `DOCKER_IMAGE` line; the variable is now `COMPUTER_IMAGE`
(see `.env.example`).
