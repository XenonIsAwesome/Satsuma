# Visual identity — "Citrus"

Satsuma's default visual identity: a light, frosted-glass "citrus" look inspired by Tangerine's general marketing-site mood (warm minimal layout, a radial wedge menu, a single orange accent), built with Satsuma's own distinct palette and typefaces rather than any copy of Tangerine's — see the [README](../README.md)'s trademark notice. This file describes the look; see [interaction.md](interaction.md) for the behavior it wraps around.

## Overview

Warm minimalism meets frosted citrus. Satsuma's UI should feel light, quiet, and slightly playful — closer to a well-made kitchen gadget than a developer tool: a near-white page, a confident single-accent color, plenty of whitespace, and a radial "wedge" menu rendered as soft frosted-glass petals around a solid-colored hub. Satsuma uses its own distinct palette and typefaces throughout, not Tangerine's actual brand colors or fonts — the two apps should feel like siblings in the same fruit family, not the same paint swatch.

The personality is calm and unhurried on the surface (drop a file, nothing else moves) but rewards a modifier key with an immediate, tactile fan of options.

This file describes the **default theme ("Citrus")**. Satsuma ships this as one of several selectable presets plus a fully custom option — see [Themes](#themes) at the end of this file. Every component in [Components](#components) is written against the semantic token names below (`colors.primary`, `colors.surface`, etc.), not literal hex values, specifically so a theme switch only has to remap those tokens — components never need their own per-theme logic.

## Design tokens

```yaml
colors:
  primary: "#FF7A29"
  on-primary: "#FFFFFF"
  primary-active: "#E85F0F"
  primary-strong: "#C24A0A"
  secondary: "#2B2620"
  on-secondary: "#FFFFFF"
  tertiary: "#7A7268"
  on-tertiary: "#FFFFFF"
  neutral: "#F6F3EE"
  surface: "#FFFFFF"
  on-surface: "#2B2620"
  on-surface-variant: "#7A7268"
  outline: "#EAE4D9"
  success: "#357A4E"
  on-success: "#FFFFFF"
  danger: "#C6432E"
  on-danger: "#FFFFFF"
typography:
  display:
    fontFamily: Plus Jakarta Sans
    fontSize: 2.8rem
    fontWeight: "600"
    lineHeight: 3rem
    letterSpacing: -0.04em
  headline:
    fontFamily: Plus Jakarta Sans
    fontSize: 1.5rem
    fontWeight: "600"
    lineHeight: 1.9rem
    letterSpacing: -0.01em
  body-md:
    fontFamily: Plus Jakarta Sans
    fontSize: 1rem
    fontWeight: "400"
    lineHeight: 1.5rem
  body-sm:
    fontFamily: Plus Jakarta Sans
    fontSize: 0.8125rem
    fontWeight: "400"
    lineHeight: 1.25rem
  label:
    fontFamily: Plus Jakarta Sans
    fontSize: 0.75rem
    fontWeight: "600"
    lineHeight: 1rem
  wedge-label:
    fontFamily: Nunito
    fontSize: 0.875rem
    fontWeight: "700"
    lineHeight: 1.1rem
rounded:
  sm: 8px
  DEFAULT: 10px
  lg: 16px
  pill: 999px
spacing:
  unit: 8px
  container-padding: 24px
  card-gap: 16px
  section-margin: 48px
```

## Colors

A warm, satsuma-inspired palette: a yellower, less-red orange than Tangerine's own brand color, paired with warm (not cool-gray) neutrals, plus `success`/`danger` for conversion-result toasts.

- **Primary (#FF7A29):** The single brand accent — buttons, the active/selected wedge petal, links, focus rings. Used sparingly; if more than one element on screen is orange without user interaction driving it, it's overused.
- **Primary Active (#E85F0F):** A deeper, more saturated orange used only for momentary interaction states — a hovered/selected wedge petal, a pressed button — never for resting UI.
- **Primary Strong (#C24A0A):** A darkened, WCAG-AA-safe stand-in for `primary`/`primary-active` (4.91:1 against white), for any place a filled-orange background carries small or non-bold white text and strict AA compliance matters more than the brand orange's exact hue.
- **Secondary (#2B2620):** Warm near-black ink for headings and primary body text — a soft brown-black, never pure `#000` or a cool gray-black.
- **Tertiary (#7A7268):** Muted warm taupe-gray for captions, helper text, and disabled labels.
- **Neutral (#F6F3EE):** The page/window background — a warm ivory, not a cool light gray, so surface cards can read as "raised" without a shadow doing all the work.
- **Surface (#FFFFFF):** Cards, tabs, the drop zone, and dialogs sit on true white against the neutral page background.
- **Outline (#EAE4D9):** Hairline borders and dividers — a warm sand tone, barely-there, never a hard gray line.
- **Success (#357A4E) / Danger (#C6432E):** Reserved for the post-conversion toast (see [interaction.md](interaction.md)) — a completed conversion vs. a reported failure. Kept in the same warm, slightly muted family as the rest of the palette rather than a saturated traffic-light green/red, and both pass 4.5:1 against white text.

**A known, deliberate contrast trade-off:** `primary`/`primary-active` on white text measures 2.60:1/3.46:1 — below WCAG AA's 4.5:1 normal-text minimum, and `primary` alone even falls short of the 3:1 large-text minimum on its own, so any label sitting directly on `primary` (not `primary-active`) should be bold and as large as the type scale allows, or should use `primary-strong` instead. This is a conscious trade-off to keep the brand orange itself warm and light rather than darkening it into AA compliance across the board — see [Do's and Don'ts](#dos-and-donts).

## Typography

**Plus Jakarta Sans** for everything except the wedge menu's own labels — a warm, geometric sans with rounded terminals. It reads confidently at large display sizes and stays legible at small caption sizes.

The wedge menu itself uses a separate, rounder display face — **Nunito** — for its petal labels (PNG, COMPRESS, etc.), reserved for that one context so the wedge's "special moment" stays typographically distinct from the rest of the app. A dedicated rounded face there also sidesteps relying on any OS-specific system font — Nunito ships bundled with the app instead, keeping Satsuma fully offline.

- **Display:** Large marketing/empty-state headlines only (e.g. a first-run screen). Tight negative letter-spacing for a confident, slightly condensed feel.
- **Headline:** Section titles within the app (Settings groups, the tools tab bar's active category).
- **Body:** Default UI text.
- **Label:** All-caps or small-caps chip/tab text (tab bar, modifier-key hint badges).
- **Wedge label:** Reserved for text rendered inside the SVG wedge menu itself — never used in regular DOM UI.

## Layout

An 8px base unit governs all spacing. The window keeps generous outer margins (24px+) so the neutral background is always visible around content — nothing should touch the window edge except the wedge menu itself, which is allowed to overflow toward the screen edge when it opens near a monitor boundary.

- **Drop zone:** Centered, large touch/click target, generous internal padding (24px+), never cramped against surrounding chrome.
- **Wedge menu:** Always centered on the trigger point (the dropped file's position, or the cursor position for the file-manager trigger path per [interaction.md](interaction.md)) — never anchored to a fixed corner or the app window's center once a launch-request trigger is involved.
- **Settings/tools panels:** Simple vertical stacks with 16px card gaps; no multi-column dense layouts — this is a small utility app, not a dashboard.

## Elevation & Depth

Depth stays subtle — Satsuma is not a dark, heavy, high-elevation UI. Two levels only:

- **Level 1 (resting surface):** `box-shadow: 0 1px 4px rgba(0, 0, 0, 0.05)` — used for tabs, cards, the drop zone. Barely perceptible; its job is separation from the neutral background, not drama.
- **Level 2 (accent controls):** `box-shadow: 0 2px 4px rgba(150, 68, 10, 0.10), inset 0 1px 0 rgba(255, 255, 255, 0.18)` — the orange primary button's shadow, giving it a faint warm glow plus a top inner highlight for a soft glossy edge. Reserved for primary-colored controls only.
- **The wedge menu** is its own elevation system, see [Shapes](#shapes) and [Components](#components) — it floats above everything else in the window (or above the desktop, for the borderless overlay trigger path) and should always render above any Level 1/2 surface.

## Shapes

- **Buttons:** Full pill (`rounded.pill`, 999px).
- **Cards, tabs, dialogs:** `rounded.sm` (8px) for small chips/tabs, `rounded.DEFAULT` (10px) for cards, `rounded.lg` (16px) for larger containers like the drop zone.
- **Wedge petals:** Not a CSS-radius shape — each petal is an SVG annulus segment (an outer arc, two radial edges, an inner arc — see `describeWedge` in the codebase) with a small angular gap between petals and a softly rounded outer corner, arranged around a circular hub. The hub itself is a plain circle. Treat the wedge as a bespoke shape family, not an extension of the rounded-corner scale above.

## Components

### Wedge menu

The centerpiece component, and the one place the UI departs from flat surfaces:

- **Hub:** A circular center showing the source file's own format/category, dimmed and non-interactive (it's excluded from the target list per [interaction.md](interaction.md)'s capability-aware-menu rule) — soft surface fill, muted text.
- **Petals (resting):** Frosted-glass look — semi-transparent white fill (~75% opacity) over whatever is behind the menu (the app window, or the desktop for the overlay trigger), with a barely-visible cool-gray stroke. Label centered, wedge-label typography, bold.
- **Petals (hover/selected):** Snap to solid `primary-active` fill with white text — the one moment saturated orange dominates the screen, deliberately, since it's the exact spot the user's cursor already is.
- **Petals (disabled):** e.g. a format the current engine can't produce — lower-opacity frosted fill, muted/outline-colored text, not clickable. Its low contrast against the frosted fill is intentional (WCAG's inactive-UI-component exception applies), not an oversight.
- **Live label:** A small pill above the hub echoes the hovered petal's action in words ("Convert to PNG") — text only, no icon, label typography on a surface background.
- **Modifier-key hint badges:** Small rounded chips showing the relevant key glyph (⇧/⌥/Ctrl/Win depending on platform and the user's remapped modifiers per [interaction.md](interaction.md)) plus a short phrase ("Hold Alt for advanced tools"). Only shown the first few times a user encounters a given trigger, not permanently — this is an onboarding aid, not a persistent HUD.

### Buttons & tabs

- **Primary button:** The only pill-shaped, solid-orange control in the app. One per screen, ideally.
- **Ghost button:** Secondary actions (Cancel, Settings), no fill, ink-colored text so it stays legible as a clickable control without competing with the primary orange.
- **Badge:** Small filled-orange status chips (e.g. the active theme's name in Settings, a "New" tag) using `primary-strong` rather than `primary`, since badge text is small and needs to hold AA contrast on its own.
- **Category tabs** (Images/Video/Audio/Documents/Archives): small pill/rounded chips in a horizontal row, active tab fills solid orange rather than just underlining.

### Drop zone

A large rounded card, resting on Level 1 elevation, that swaps to a faint neutral-tinted fill the instant a drag is over the window (never solid orange — that's reserved for confirmed selection, not a hover/drag-over state).

### Toasts

The post-conversion feedback state from [interaction.md](interaction.md) ("Progress & completion feedback"): a small toast anchored near the drop zone or wedge origin point, auto-dismissing on success, persistent (with the source filename and error detail) on failure.

## Do's and Don'ts

- **Do** keep orange rare and earned — it should mean "this is the thing you can act on right now," never decorative.
- **Do** keep the wedge's frosted-petal look consistent across both trigger paths (in-window and the borderless desktop overlay from [interaction.md](interaction.md)) — it's the same component, just rendered in a transparent host window in the overlay case.
- **Do** keep Satsuma's palette and typefaces (Plus Jakarta Sans, Nunito, the satsuma-orange `#FF7A29` family) distinct from Tangerine's actual brand colors and fonts — the resemblance should be in mood and layout, never in literal hex values or font names, matching the non-affiliation notice in the [README](../README.md).
- **Don't** use the `primary-active` hot-orange for anything that isn't a momentary interaction state — if it's visible while nothing is hovered/pressed, it's the wrong token.
- **Don't** add a second accent color to the default Citrus theme. If a feature seems to need one, it's a candidate for the [Themes](#themes) system below, not a new token in the default palette.
- **Don't** give the wedge menu a drop shadow as heavy as a typical modal — it should feel like it's floating in frosted glass, not sitting in front of a dark scrim.

## Themes

Satsuma's Settings adds a **theme picker**: a small set of built-in presets, each a full remap of the semantic color tokens above (`colors.*` — never `typography`/`rounded`/`spacing`, which stay constant across themes so component layout never shifts when the theme changes), plus a **Custom** option.

Every preset below only lists tokens that differ conceptually from Citrus's role assignments — `on-*` pairing tokens (e.g. `on-primary`, `on-surface`) always flip to whichever of black/white keeps ≥4.5:1 contrast against their paired color.

| Theme | primary | primary-active | secondary | neutral | surface | on-surface | outline |
|---|---|---|---|---|---|---|---|
| **Citrus** (default, light) | `#FF7A29` | `#E85F0F` | `#2B2620` | `#F6F3EE` | `#FFFFFF` | `#2B2620` | `#EAE4D9` |
| **Midnight Citrus** (dark) | `#FF9452` | `#FFAB74` | `#F5EFE6` | `#1B1815` | `#262220` | `#F5EFE6` | `#3A342E` |
| **Yuzu** (light, mustard-yellow accent) | `#D4A017` | `#B8890A` | `#26241C` | `#F7F5EC` | `#FFFFFF` | `#26241C` | `#E8E2D0` |
| **Blood Orange** (light, deeper red-orange) | `#C4432A` | `#A5361F` | `#241914` | `#F5EEEA` | `#FFFFFF` | `#241914` | `#E6D9D2` |
| **Custom** | user-chosen | derived (auto-darkened/saturated from `primary`) | user-chosen | user-chosen | user-chosen | derived (auto black/white for contrast) | derived (auto, from `neutral`) |

**Midnight Citrus** exists because Satsuma, unlike Tangerine, runs on platforms where a system-wide dark mode is a first-class, commonly-enabled setting (Windows and most Linux desktops). Note its interaction-state relationship inverts versus the light themes: `primary-active` gets *brighter*, not darker, since a darker orange would lose contrast against a near-black background. **Yuzu** and **Blood Orange** are lighter-touch identity variants for users who want a different fruit's mood without leaving the "citrus" family Satsuma's own branding is built on.

### Custom theme behavior

Selecting **Custom** in Settings exposes color pickers bound directly to the same five user-set tokens every preset defines (`primary`, `secondary`, `neutral`, `surface`, and `outline`) — never more than that, so a user can't accidentally break a component by overriding a token they don't understand the role of. Derived tokens (`primary-active`, every `on-*` pairing) are computed automatically:

- `primary-active` — the user's `primary`, programmatically pushed ~10% darker and ~10% more saturated for a light theme (or lighter, for a dark-background custom theme — detected from the chosen `neutral`'s own lightness), the same relationship Citrus's `#FF7A29` → `#E85F0F` demonstrates.
- Every `on-*` token — computed at run time as pure black or pure white, whichever gives the higher WCAG contrast ratio against its paired color, so a Custom theme can never produce unreadable text no matter what the user picks.
- `outline` if not explicitly set — derived from `neutral` (a fixed lightness/darkness offset), though it's included as a picker above since border visibility is easy to get wrong by accident with a fully-derived value.

The theme picker (including Custom's pickers) lives in Settings, applies instantly with no restart, and persists per-user (not per-file or per-conversion) — it's a personalization setting, not part of the document/file model.
