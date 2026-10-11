# CRT TUI Animation Pattern

Techniques used in `assets/tui.html` for the retro terminal splash screen.

## Visual Stack

| Layer | Technique | CSS |
|---|---|---|
| Scanlines | `repeating-linear-gradient` on `::before` pseudo-element | `background: repeating-linear-gradient(0deg, transparent, transparent 2px, rgba(0,255,65,0.04) 2px, ...)` |
| Vignette | `radial-gradient` on `::after` pseudo-element | `background: radial-gradient(ellipse at center, transparent 60%, rgba(0,0,0,0.6) 100%)` |
| Flicker | `animation` on `body` with tiny opacity oscillation | `animation: crt-flicker 0.05s infinite alternate` |
| Glow | `text-shadow` with increasing blur radii | `text-shadow: 0 0 10px var(--green-bright), 0 0 30px rgba(57,255,20,0.4)` |
| Breathing | `transform: scale()` + `text-shadow` intensity cycle | `@keyframes` with 3s `ease-in-out` infinite |

## Blink Implementation (JS, not CSS)

CSS `opacity` animations on inline elements inside `<pre>` don't render visibly with monospace fonts in all browsers. Use **JS class-swapping** with two `<span>` elements:

```html
<span class="eye-open">◉</span>
<span class="eye-closed">─</span>
```

```css
.eye-open { display: inline; }
.eye-closed { display: none; }
.face.blink .eye-open { display: none; }
.face.blink .eye-closed { display: inline; }
```

```js
function blink() {
  document.getElementById('face').classList.add('blink');
  setTimeout(() => document.getElementById('face').classList.remove('blink'), 150);
}
function scheduleBlink() {
  blink();
  setTimeout(scheduleBlink, 2000 + Math.random() * 3000);
}
scheduleBlink();
```

**Why this works:** The `display: none/inline` swap forces the layout engine to reflow, unlike `opacity` which can be optimized away. 150ms close duration matches human blink timing. Random 2-5s interval prevents predictability.

## Tear Drip

```css
@keyframes tear-drip {
  0% { opacity: 0; transform: translateY(0); }
  30% { opacity: 1; }
  70% { opacity: 1; }
  100% { opacity: 0; transform: translateY(20px); }
}
```

Position tears with `position: absolute` on the face container.

## Glitch Effect

```css
@keyframes glitch-shift {
  0%, 100% { transform: translateX(0); clip-path: none; }
  20% { transform: translateX(-2px); clip-path: inset(20% 0 40% 0); }
  40% { transform: translateX(2px); clip-path: inset(60% 0 10% 0); }
  60% { transform: translateX(-1px); clip-path: inset(10% 0 70% 0); }
  80% { transform: translateX(1px); clip-path: inset(40% 0 30% 0); }
}
```

Trigger on critical findings: `element.style.animation = 'glitch-shift 0.15s steps(1)'`, then clear after 150ms.

## Event Log Auto-Scroll

```js
eventsLog.scrollTop = eventsLog.scrollHeight;
```

Hide old entries with `:nth-child(n+6) { display: none; }` to keep the log bounded.

## Browser Gotcha

`browser_navigate` blocks `file://` URLs. Always serve local HTML files with `python3 -m http.server <port>` and navigate to `http://localhost:<port>/file.html`.
