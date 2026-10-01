# Decisions

Every decision with lasting impact, newest first. Each entry says what was decided, why, and what else was
considered. When experience contradicts a decision, add a dated amendment instead of rewriting history.

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
