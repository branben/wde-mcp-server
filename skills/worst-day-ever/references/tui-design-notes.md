# TUI Design Notes

Design rationale and anti-patterns for the worst-day-ever splash screen.

## Design System Choice

The TUI uses the **Linear** design system. Linear is the right reference because:

- It is a **dark-native** product (not a light theme with dark mode bolted on)
- It uses **luminance-based elevation** (not shadow-based) — the correct approach for dark surfaces
- Typography is **Inter + JetBrains Mono** — the standard developer-tool font pairing
- It avoids decorative noise — every element earns its place
- Borders are `rgba(255,255,255,0.05–0.08)` — whisper-thin, not heavy

## Color Palette

| Token | Value | Role |
|---|---|---|
| `--bg-canvas` | `#08090a` | Deepest background |
| `--bg-panel` | `#0f1011` | Card/panel backgrounds |
| `--bg-surface` | `#191a1b` | Elevated surfaces |
| `--border-subtle` | `rgba(255,255,255,0.05)` | Default separators |
| `--border-standard` | `rgba(255,255,255,0.08)` | Card borders |
| `--text-primary` | `#f7f8f8` | Headings, key values |
| `--text-secondary` | `#d0d6e0` | Body text |
| `--text-tertiary` | `#8a8f98` | Labels, metadata |
| `--text-quaternary` | `#62666d` | Timestamps, dim text |
| `--hostile` | `#ff4444` | Critical findings |
| `--warning` | `#f0a030` | Warnings, detected issues |
| `--ok` | `#10b981` | Pass, safe, accepted |
| `--accent` | `#7170ff` | Interactive accent (cursor) |

## What to Avoid (Anti-Patterns)

1. **Retro CRT aesthetics** — scanlines, phosphor glow, green-on-black. These signal "hacker movie" not "production tool." Instead: near-black canvas, subtle scanlines as texture (not animation), precise typography.

2. **Emoji as primary content** — The first version used `💧` tears. Emoji render inconsistently across platforms and break the monospace grid. Use ASCII/Unicode block characters instead (`░▒▓█`).

3. **CSS-only blink** — `animation: opacity` on `<span>` inside `<pre>` doesn't render reliably across browsers. Use **JS class-toggle** for blink: add/remove `.blink` class, let CSS handle the visual swap.

4. **Unreadable ASCII art titles** — Small `font-size: 10px` ASCII art banners are illegible. If you need a text title, use real typography (42px, weight 600, negative letter-spacing). Reserve ASCII art for decorative elements at 14px+.

5. **Glitch as constant animation** — Random glitch every 3s is noise. Trigger glitch only on meaningful events (critical finding detected).

## Animation Rules

| Element | Animation | Timing | Trigger |
|---|---|---|---|
| Face eyes | `◉`→─ swap | 120ms duration, every 2.5-5.5s | Random (JS interval) |
| Face card | translateX(-3px, 3px) | 150ms | Critical finding only |
| Event rows | translateY(4px→0) + fade in | 300ms ease-out | On insert |
| Cursor block | opacity blink | 1s step-end infinite | Always |
| Status dot | opacity pulse | 1.5s ease-in-out infinite | Always |

## Typography

- **Display title:** Inter 42px weight 600, letter-spacing -1.5px, color `--text-primary`
- **Section labels:** JetBrains Mono 10px uppercase, letter-spacing 0.5px, color `--text-quaternary`
- **Body:** Inter 13-14px weight 400, color `--text-secondary`
- **Mono values:** JetBrains Mono 12px, color `--text-tertiary`

## Layout

- Max width: 760px (comfortable reading, not stretched)
- 12px border radius on cards (not pill, not sharp)
- 1px borders with `var(--border-subtle)` — barely visible
- Grid: CSS grid for status cells (3 columns desktop, 1 mobile at 640px)
- Event log: max-height 200px, overflow-y auto, custom 4px scrollbar

## Responsive

- Mobile (<640px): single-column status grid, smaller title (32px), reduced padding
- No horizontal scroll at any breakpoint
- Event log maintains scroll position on new events
