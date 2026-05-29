---
version: alpha
name: JBrowser
description: >
  Remote browser fleet management with a warm editorial aesthetic. Parchment
  canvas (#efe7d2), single coral-red accent (#ed6f5c), Inter Tight 800-weight
  display type. Three surfaces sharing one warm palette: split-panel login,
  sidebar dashboard, and an immersive browser detail view for real-time remote
  control.

colors:
  bg: "#efe7d2"
  surface: "#f7f1de"
  surface-warm: "#ece4cf"
  surface-dark: "#ddd2b6"
  fg: "#15140f"
  fg-2: "#2a2620"
  muted: "#5a5448"
  meta: "#8b8676"
  border: "rgba(21,20,15,0.12)"
  border-soft: "rgba(21,20,15,0.07)"
  accent: "#ed6f5c"
  accent-on: "#ffffff"
  accent-soft: "#f08e7c"
  success: "#2d6a4f"
  warn: "#b47d1e"
  danger: "#b5341a"
  brand-bracket: "#8b8676"
  brand-j: "#ed6f5c"
  brand-browser: "#15140f"

typography:
  hero-display:
    fontFamily: "'Inter Tight', system-ui, sans-serif"
    fontSize: clamp(48px, 8vw, 88px)
    fontWeight: 800
    lineHeight: 0.94
  heading-1:
    fontFamily: "'Inter Tight', system-ui, sans-serif"
    fontSize: clamp(30px, 3vw, 40px)
    fontWeight: 800
    lineHeight: 1.1
    letterSpacing: -0.025em
  heading-2:
    fontFamily: "'Inter Tight', system-ui, sans-serif"
    fontSize: 34px
    fontWeight: 800
    lineHeight: 1.15
    letterSpacing: -0.025em
  heading-3:
    fontFamily: "'Inter Tight', system-ui, sans-serif"
    fontSize: 15px
    fontWeight: 600
    lineHeight: 1.3
  eyebrow:
    fontFamily: "'Inter Tight', system-ui, sans-serif"
    fontSize: 11px
    fontWeight: 600
    letterSpacing: 0.22em
    textTransform: uppercase
  body-md:
    fontFamily: "'Inter', system-ui, sans-serif"
    fontSize: 14px
    fontWeight: 400
    lineHeight: 1.55
  body-sm:
    fontFamily: "'Inter', system-ui, sans-serif"
    fontSize: 13px
    fontWeight: 500
    lineHeight: 1.5
  body-sm-bold:
    fontFamily: "'Inter Tight', system-ui, sans-serif"
    fontSize: 13px
    fontWeight: 700
    lineHeight: 1.0
  caption:
    fontFamily: "'Inter', system-ui, sans-serif"
    fontSize: 12px
    fontWeight: 400
    lineHeight: 1.4
  caption-bold:
    fontFamily: "'Inter Tight', system-ui, sans-serif"
    fontSize: 11px
    fontWeight: 600
    lineHeight: 1.0
    letterSpacing: 0.22em
    textTransform: uppercase
  micro:
    fontFamily: "'JetBrains Mono', ui-monospace, monospace"
    fontSize: 11px
    fontWeight: 500
    lineHeight: 1.0
  mono:
    fontFamily: "'JetBrains Mono', ui-monospace, monospace"
    fontSize: 12px
    fontWeight: 500
    lineHeight: 1.0
  brand-name:
    fontFamily: "'Inter Tight', system-ui, sans-serif"
    fontSize: 18px
    fontWeight: 700
    lineHeight: 1

rounded:
  sm: 4px
  md: 8px
  lg: 12px
  xl: 18px
  pill: 9999px

spacing:
  xs: 4px
  sm: 8px
  md: 12px
  lg: 16px
  xl: 20px
  xxl: 24px
  xxxl: 32px
  sidebar-w: 224px

components:
  btn-primary:
    backgroundColor: "{colors.accent}"
    textColor: "{colors.accent-on}"
    typography: "{typography.body-sm-bold}"
    rounded: "{rounded.pill}"
    padding: "12px 18px"
  btn-create:
    backgroundColor: "{colors.accent}"
    textColor: "{colors.accent-on}"
    typography: "{typography.body-sm-bold}"
    rounded: "{rounded.pill}"
    padding: "8px 18px"
  input-field:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.fg}"
    typography: "{typography.body-md}"
    rounded: "{rounded.md}"
    padding: "12px 14px"
  login-card:
    backgroundColor: "{colors.surface}"
    rounded: "{rounded.xl}"
    padding: "34px"
  browser-card:
    backgroundColor: "{colors.surface}"
    rounded: "{rounded.lg}"
    padding: "20px"
  status-badge:
    rounded: "{rounded.pill}"
    padding: "4px 10px"
    typography: "{typography.caption-bold}"
  nav-item:
    height: 36px
    rounded: "{rounded.md}"
    textColor: "{colors.muted}"
    typography: "{typography.body-sm}"
  filter-tab:
    textColor: "{colors.meta}"
    typography: "{typography.body-sm-bold}"

shadow:
  card: "0 1px 3px rgba(21,20,15,0.06), 0 0 0 1px rgba(21,20,15,0.08)"
  raised: "0 4px 16px rgba(21,20,15,0.10), 0 0 0 1px rgba(21,20,15,0.08)"

motion:
  fast: 150ms
  base: 240ms
  ease: "cubic-bezier(0.2,0,0,1)"
---

## Overview

JBrowser's interface draws from print design — warm parchment canvas, a single coral-red voltage, and Inter Tight headlines at 800 weight. The goal is an editorial-grade control plane that feels handcrafted rather than SaaS-generic. Depth comes from layered warm surfaces with `color-mix(in oklab, ...)` tinting rather than heavy shadows.

The brand identity is a bracketed `[J]BROWSER` logotype: warm gray brackets framing a coral "J" and near-black "BROWSER", accompanied by a 16×16px coral square mark.

The app has three distinct surfaces, each with its own visual treatment:

1. **Login** — Split-panel (55%/45%) with oversized italic hero headline, feature bullets, and a card-based form. The warm parchment palette at full expression.
2. **Dashboard** — 224px warm sidebar + browser card grid. Status badges with animated dots, search, filter tabs (All/Online/Offline), 3-column metric grids per card.
3. **Browser Detail** — Immersive browser view sharing the same warm palette. Chrome-style tab strip on `{colors.surface-warm}`, toolbar on `{colors.bg}`, canvas area on `{colors.surface-dark}`, and status bar with CDP token management.

**Key Characteristics:**
- Single accent color: coral `{colors.accent}` carries every CTA, active state, and brand moment
- Warm-tinted rgba shadows instead of pure black — the shadows feel organic
- Pill shape for all interactive buttons and badges — friendly, tactile feel
- Extreme type weight contrast: 800 display / 500 body / 700 buttons
- Unified warm palette across all three surfaces — no dark/light mode split
- `color-mix(in oklab, ...)` generates all tinted backgrounds dynamically

## Colors

### Why this palette
The warm parchment base (`{colors.bg}`) makes screens feel like quality paper rather than generic white. A single coral accent (`{colors.accent}`) does all the brand work — used sparingly so it never dilutes. This mirrors editorial design where one spot color on off-white stock creates maximum impact.

### Semantic roles
- **Accent** — `{colors.accent}` is the only brand color. CTAs, active nav, links, progress bar, ripple effects. Every primary interaction runs through coral.
- **Surfaces** — Four warm tiers from light (`{colors.surface}`) to dark (`{colors.surface-dark}`). The sidebar and browser tab strip use `{colors.surface-warm}`; the browser canvas area uses `{colors.surface-dark}` to create contrast behind the page preview.
- **Text** — Four-level hierarchy (`{colors.fg}` → `{colors.meta}`) provides clear reading order without resorting to size changes alone.
- **Borders** — Semi-transparent warm black (`rgba(21,20,15,...)`) instead of named grays. This keeps borders in harmony with any background they sit on.
- **Status** — Forest green (`{colors.success}`), amber (`{colors.warn}`), brick red (`{colors.danger}`). Each mixes at 12% into the surface for tinted badge backgrounds.

## Typography

Font stack: **Inter Tight** (display/headings/buttons), **Inter** (body), **JetBrains Mono** (metrics/status/code).

| Level | Size | Weight | Use |
|---|---|---|---|
| hero-display | 48–88px | 800 | Login hero headline (fluid via `clamp()`) |
| heading-1 | 30–40px | 800 | Page titles |
| heading-2 | 34px | 800 | Login card heading |
| heading-3 | 15px | 600 | Card names |
| eyebrow | 11px | 600 | Uppercase labels (0.22em tracking) |
| body-md | 14px | 400 | Body text, descriptions |
| body-sm | 13px | 500 | Nav items, secondary text |
| body-sm-bold | 13px | 700 | Buttons, filter tabs |
| caption | 12px | 400 | Timestamps, user meta |
| caption-bold | 11px | 600 | Status badges (uppercase) |
| micro / mono | 11–12px | 500 | Count badges, metrics, status bar |

**Why these choices:** Inter Tight at 800 with negative letter-spacing (-0.025em) creates the editorial "headline set in heavy ink" feel. Body stays at Inter 400–500 for reading comfort. JetBrains Mono for anything technical — it signals "this is machine data" without needing an explicit label.

## Layout

Three layout systems, one per surface:

1. **Login** — CSS Grid `55% / 45%`, full viewport height. Left panel: brand + hero + features. Right panel: centered login card.
2. **Dashboard** — Flexbox: 224px fixed sidebar + fluid main. Main area: page shell (30px 34px 40px padding) with `repeat(auto-fill, minmax(300px, 1fr))` card grid, 18px gap.
3. **Browser Detail** — Flexbox column, 100dvh. Stack: tab strip (40px) → toolbar (42px) → progress (2px) → canvas (flex:1) → status bar (22px).

## Elevation

Two levels only. Warm-tinted (`rgba(21,20,15,...)`) instead of pure black.

| Level | Use | Interaction |
|---|---|---|
| `{shadow.card}` | Browser cards, panels | Default resting state |
| `{shadow.raised}` | Login card, hovered cards | Cards lift `translateY(-2px)` on hover, buttons lift `-1px` |

## Components

**`btn-primary`** — Coral pill CTA. Hover: lifts `-1px`, softens to `{colors.accent-soft}`. The pill shape and lift make it feel physically pressable.

**`browser-card`** — Warm surface card with `{shadow.card}`. Contains name + status badge + 3-column metric grid. On hover: lifts, border tints coral 30%, shadow upgrades to `{shadow.raised}`.

**`status-badge`** — Pill with 7px animated dot + uppercase label. Three variants (online/offline/restarting) use `color-mix` to tint the surface at 12% of the semantic color. The dot subtly animates for "online" state.

**`input-field`** — Warm surface input. Focus: border blends coral via `color-mix(in oklab, accent 45%, border)`, 3px ring at 12% opacity. The accent bleeds in gently rather than snapping to a hard focus color.

**`nav-item`** — 36px sidebar link with 16px stroke icon. Active: coral text + coral-tinted background via `color-mix`. The tint keeps the active state feeling warm, not highlighted.

**`filter-tab`** — Bottom-border tab strip. Active: coral text + 2px coral underline. No background change — the underline alone carries the signal.

### Browser Detail components
- **Tab strip** — Chrome-style tabs on `{colors.surface-warm}`: rounded `8px 8px 0 0`, active tab matches `{colors.bg}` so it "opens" into the toolbar.
- **Address bar** — Pill input (`rounded: 20px`), `{colors.surface}` background, coral focus border.
- **Connection dot** — 8px circle, `{colors.meta}` → `{colors.success}` with 300ms transition.
- **Progress bar** — 2px coral sliding animation (1.4s ease-in-out infinite) when loading.
- **Click ripple** — 28px coral circle, scales 0.3→2.2 then fades over 550ms. Confirms that clicks are registering on the remote canvas.
- **Canvas area** — `{colors.surface-dark}` background provides contrast for the JPEG preview without jarring the warm palette.
- **Status bar** — 22px, `{colors.surface-warm}` background, truncated URL left, agent info + CDP links right.

## Responsive Behavior

| Breakpoint | Login | Dashboard | Browser Detail |
|---|---|---|---|
| ≤ 920px | Single column, divider hidden | Sidebar → horizontal top bar | Unchanged (already full-screen) |

## Do's and Don'ts

**Do:**
- Use `{colors.accent}` only for primary actions and active states — scarcity creates impact
- Use `color-mix(in oklab, ...)` for all tinted backgrounds — never hand-pick tint hex values
- Keep shadows warm-tinted (`rgba(21,20,15,...)`) — never pure black
- Use pill shape for all interactive buttons and badges

**Don't:**
- Don't add a second accent color — the single-coral constraint is the brand
- Don't mix Inter Tight and Inter within the same text block
- Don't use more than 4 text color levels in a single view

## Known Gaps

- Animation/transition timings beyond `{motion.fast}` / `{motion.base}` are not specified — use best judgment
- Mobile layouts below 920px are minimal; small-screen dashboard UX needs further design
- Error/success toast patterns are not documented
- Multi-language / RTL behavior is not covered
