# Color schemes

People pick one of ten color schemes in **Settings › Appearance**. Every scheme has a light and a dark version, and
the existing mode setting (Light, Dark, Match my system) chooses between them. Schemes change colors only: fonts,
spacing and layout stay the same in every scheme.

Design: https://claude.ai/artifact/QQYkK2wafZr5RWor5aVzdJ (the picker across the top recolors every screen on the
canvas; the bottom row is the Settings › Appearance screen to build).

## The data

| File | What |
| --- | --- |
| `crates/server/catalog/schemes.json` | The order the picker shows schemes in, and the default (`croncave`) |
| `crates/server/catalog/scheme-<id>.json` | One scheme: `id`, `name`, `description`, `version`, and every token with `dark` and `light` values |

The ten, in picker order:

| id | Name | Description |
| --- | --- | --- |
| `croncave` | Croncave | Lime on graphite (unchanged; still the default) |
| `ember` | Ember | Terracotta on paper |
| `harbor` | Harbor | Deep blue, cool greys |
| `fern` | Fern | Leaf green, soft sage |
| `plum` | Plum | Violet, lavender greys |
| `rose` | Rose | Raspberry, blush |
| `lagoon` | Lagoon | Teal, sea-glass greys |
| `saffron` | Saffron | Amber, warm cream |
| `graphite` | Graphite | Black and white |
| `contrast` | High contrast | Sharpest text and edges |

Every scheme has exactly Croncave's token set and passes `schemeProblems()` in `web/src/lib/theme/theme.ts` in both
modes (text 4.5:1 on every background, statuses readable and distinct from each other and from the accent). Treat
the files as the source of truth: to adjust a color, edit the JSON, bump `version`, and keep that check green.

## What to build

1. **Seed every scheme.** `seed.rs` inserts each file named in `schemes.json` (an `include_str!` per file), never
   overwriting an existing `(id, version)`, the way it seeds Croncave today.
2. **List them.** `GET /api/schemes` returns `{ "default": "croncave", "schemes": [ … ] }`, each the latest version of
   a scheme, in `schemes.json` order. `GET /api/schemes/{id}` stays as it is. Both are public (the web server reads
   them before anyone signs in).
3. **Save the choice.** `POST /api/me/prefs` accepts `scheme`; an id that isn't in the catalog is refused with
   "That color scheme doesn't exist." New users get the default (already `"scheme": "croncave"` in `auth.rs`).
4. **First paint.** `hooks.server.ts` reads a `cc_scheme` cookie next to `cc_mode`, accepts only `^[a-z0-9-]{1,40}$`,
   and serves that scheme's CSS through `schemeCss()`. Cache each scheme's CSS for a minute, as Croncave's is now,
   and bundle every scheme file as the fallback (`import.meta.glob(..., { eager: true })`), so an unknown or missing
   scheme falls back to Croncave and nothing ever paints unstyled.
5. **The picker.** Replace the disabled "Color scheme" select in `routes/(app)/settings/+page.svelte` with the
   design's picker:
   - Mode as a segmented control: Light, Dark, Match my system (same behavior as the radios today).
   - A radio group of ten cards, five per row. Each card shows the scheme in miniature, light half on the left
     and dark half on the right: the background, a small card with an `ink` line and a `mid` line, four status dots
     (`live`, `working`, `needs`, `failed`) and an `accent` bar. Under it are the name, the description, and a radio
     that becomes a filled check on the chosen card, whose border turns `accent`.
   - The swatches are the only place a component shows another scheme's colors. Pass them from the scheme data as
     CSS custom properties on the card (`style="--p-bg: …"`) and style with `var(--p-…)`. The values are data, so
     the "no hex values in components" rule still holds.
   - Choosing a card applies it at once: replace the text of `<style id="cc-theme">` with that scheme's
     `schemeCss()`, set the `cc_scheme` cookie for a year, save `{ scheme }`, and show the "Appearance saved" toast.
   - Under the picker, a "Preview: <name>" section shows the chosen scheme light and dark side by side: four rows
     with the statuses Awake, Working, Needs you and Failed (each a colored word on a soft fill) and a "Run now"
     button in the accent.
   - It's a real radio group: arrow keys move between cards, and the checked card has `aria-checked="true"`.
6. **Follow the person between devices.** After `/me` loads, if `prefs.scheme` or `prefs.mode` differ from the
   cookies, apply them the same way and update the cookies.
7. **Measure.** Record `appearance.scheme_changed` with the old and new ids, so we learn which schemes people keep.

## Tests

- **Vitest:** every id in `schemes.json` has a file; every file passes `schemeProblems()`; every file has the same
  token names as Croncave; no scheme file is missing from the index.
- **Rust:** `POST /me/prefs` with an unknown scheme is a 400; the list endpoint returns ten schemes in order.
- **Playwright** (in the settings or sign-up flow): pick Harbor, check that `--accent` on `<html>` is Harbor's value
  for the current mode, reload and check that it stuck, then sign in from a fresh browser context and see it
  follow you.
