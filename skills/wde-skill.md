---
name: worst-day-ever
description: |
  Adversarial stress-testing skill — generate worst-case user flows, execute them
  safely in a sandbox, and produce a rendered HTML disaster report with ELI5
  explanations, severity-graded findings, and agent-to-agent frontmatter.

  Use when the user says any of:
  - "worst day ever" / "WDE" / "wde"
  - "adversarial test" / "adversarial assessment"
  - "stress test" / "torture test" / "break my project"
  - "what could go wrong" / "edge case simulation"
  - "disaster report" / "disaster check"
  - "run the assessment" / "run WDE"
  - Any request to systematically test edge cases, input validation, state
    machine, gate logic, authZ, lifecycle, or error handling
  - "show me the TUI" — launch `assets/tui.html` as a visual splash

  Do NOT use for: general code review, static analysis, production
  penetration testing, or debug sessions.
---

# Worst-Day-Ever

A disciplined adversarial user simulation: systematically generate the worst things
a user could do, execute them safely, and report how the system responds. Never
touches real data, secrets, or shared state.

## Core Philosophy

Every system has undiscovered user flows — combinations of inputs, timings, state
transitions that no one designed for but users will find. This skill exists to
surface them *before* users do, in a controlled environment.

The goal is not to be destructive. The goal is to be *honest* about how the system
behaves under stress.

## Before You Begin

### Greenfield vs Brownfield

The workflow diverges based on whether the project has real traffic data:

**Greenfield (no production traffic / pre-launch)**

Input sources: API specs (OpenAPI/GraphQL schema), route definitions, state
machine docs, README, type definitions.

Attack surface is **derived from contracts**:
- What types does this endpoint accept? → violate them
- What states exist? → traverse them out of order
- What are the rate limits? → exceed them
- What fields are optional? → omit them. What fields are required? → send garbage.

**Brownfield (has production traffic / live system)**

Input sources: production logs (sanitized), error tracking, access logs, DB
schema, APM traces.

Attack surface is **mined from real patterns**:
- Which endpoints have highest 5xx rate? → amplify the malformed input that triggered it
- Which user flows have highest drop-off? → the abandoned state is probably an orphaned record
- What's the 99th percentile latency? → send a request that will time out, then check recovery
- Which records are referenced but deleted? → follow the dangling pointer

**NEVER use real PII.** Anonymize all IDs before analysis. If mining logs,
replace every real ID with a synthetic one of the same format. Log structures
matter; content does not.

### Safeguards

These are non-negotiable. If any cannot be met, abort and tell the user why.

1. **No real data** — all inputs synthesized. Real schemas used only for type/format inference
2. **No secrets** — if the system requires auth, create a test token. Never read `.env` or keyrings
3. **Sandboxed execution** — run in isolated subprocess / DB fake / HTTP stub. No writes to shared state
4. **No external calls** — if the system hits external services (payment gateway, email API), stub them with failure modes
5. **Bounded duration** — each flow runs max 30 seconds. If it hangs, kill it and report "hang detected"
6. **No mutation side effects** — if the system writes to a screen/file, capture the write. If it writes to a DB, roll back or use a throwaway

## Attack Surface Matrix

Execute dimensions in order. For each dimension, generate 3–5 scenarios of
escalating severity. Document the result.

### Dimension 1 — Input Boundary
- Max-length strings (2× declared max)
- Null bytes embedded in text fields
- Unicode edge cases: ZWNJ (`U+200C`), RTL override (`U+202E`), 4-byte emoji, BOM at string start
- Type confusion: `{"age": "not_a_number"}`, `{"tags": "should_be_array"}`
- Deeply nested payloads: JSON depth 50+, circular `$ref` in schemas
- Empty/whitespace-only to required fields
- Arithmetic edge cases: `0`, `-1`, `MAX_INT+1`, `NaN`, `Infinity`

### Dimension 2 — State Machine Torture
- Double-submit: replay the final mutation N times (idempotency test)
- Back-button resurrection: create → confirm → nav back to confirm → submit confirm again
- Out-of-order: try confirm before create, delete after refund, approve after reject
- Rapid transitions: submit state changes faster than processing time
- Concurrent state mutations: two clients both "claim" the same resource
- Zombie state: act on resource whose state is already terminal (shipped → try cancel)

### Dimension 3 — Temporal / Timing
- Replay valid-but-expired tokens (session history hijack — use synthetic sessions)
- Clock skew payloads: `If-Modified-Since` = `2099-01-01`, `exp` = `2000-01-01`
- Counter race: 100 simultaneous requests that each claim the "last available" slot
- Connection pool depletion: send slow requests that starve the pool, then hit critical path
- Polling abuse: hammer an endpoint that triggers expensive work on each call (e.g. report generation)

### Dimension 4 — AuthN/Z Shadow
- Horizontal escalation: increment resource IDs (user_1's order → user_1/order_2, /3, /4...)
- Vertical escalation: submit `is_admin=true` or `role=admin` in payload when token says `user`
- Scope mismatch: token with `read:orders` hitting `POST /orders` or `DELETE`
- Session fixation: reuse a pre-auth token after login (server should rotate)
- Cross-tenant: if multi-tenant, hit another tenant's resources with wrong `X-Tenant-Id`
- Missing token: drop `Authorization` entirely on protected endpoints

### Dimension 5 — Data Integrity Cascade
- Delete referenced entity: user deleted → do their scheduled jobs still run? Their orders ship?
- Encoding roundtrips: submit Latin-1 bytes, store as UTF-8, request with `Accept-Charset: ISO-8859-1`
- Pagination edge cases: `?cursor=-1`, `?offset=999999999`, `?limit=0`, `?limit=-1`
- Constraint bypass: submit invalid enum that passes validation but hits DB constraint
- Schema drift: send payload from an older API version to a newer endpoint (missing required fields)
- Reference integrity: create resource pointing to non-existent parent (should 404, may 500)

### Dimension 6 — Concurrent Load Concentration
- Thundering herd: 1000 simultaneous reads on same record at exact moment it's being mutated
- Cache death spiral: expire a hot cache entry → read misses → DB stampede
- Write skew: two transactions both read overlapping data → both commit → invariant violated
- Lock escalation: acquire many row locks → escalates to table lock → blocks readers
- Hotspot abuse: hammer a cheap-to-validate, expensive-to-execute endpoint (search with sort on unindexed column)

### Dimension 7 — External Dependency Failure
- Payment gateway down → does the order get created anyway? In what state?
- Email service timeout → does the user know their action succeeded? Can they retry safely?
- CDN serving stale asset → does the app break silently or show error?
- DNS failure on external service → does it hang or fail fast?
- Third-party returns malformed response → does validation catch it or does it crash upstream?

### Dimension 8 — Brownfield-Only (Log Mining)
- Parse production logs (sanitized): find the 5 inputs that caused the worst errors → replay with variations
- Find unused enum values in schema → submit them to see if they were removed from validation
- Soft-delete audit: find records marked deleted but still referenced by active queries
- Slow query correlation: identify the slowest 1% of requests → what input pattern caused them?
- Error message quality: does the error message leak stack traces, internal paths, or schema names?

## Automated Workflow

Each assessment runs 5 phases. Execute them in order. Do NOT ask permission
between phases.

### Phase 1 — Discover

1. **Find the project root**: Look for `setup.py`, `pyproject.toml`, or the
   main source file the user points at.
2. **Map the CLI surface**: Run `<tool> --help` or inspect `main()` / `cli()`
   to enumerate all commands, flags, and subcommands.
3. **Understand core data flow**: Read the key files that handle:
   - Input parsing (argument validation, type coercion)
   - State mutations (score updates, agent registration, routing decisions)
   - Authentication/authorization (gate checks, force flags)
   - Persistence (file I/O, data loading/saving)
4. **Identify test hooks**: Can it run in a sandbox? Is there a test mode,
   dry-run flag, or isolated data directory? If not, set up a temp directory
   and environment.

### Phase 2 — Generate Scenarios

Write a single Python assessment script (`wde_assessment.py`) that tests all
applicable dimensions. The script MUST:

- Be **self-contained** — imports only stdlib + project's own modules
- Use **subprocess** to invoke the CLI (isolates state between scenarios)
- Run in a **temp directory** — never touches real data
- Collect results into the **standard results format** (see below)
- Save `wde_results.json` when done

#### Dimension Coverage Table

| Dim | Name | What to test | Always test? |
|---|---|---|---|
| 1 | Input Validation | Empty strings, NaN, Infinity, negative numbers, non-numeric types, boundary values | ✅ Always |
| 2 | State Machine | Outcome replay, gate oscillation, fail cascade, partial→fail→success chains | ✅ Always |
| 3 | Numerical Stability | EMA fixed points, float64 precision under 100+ updates, saturating behavior, success decay paradox | ✅ Always |
| 4 | Authorization | Force-assignment bypassing gates, domain-specific vs `_default` gating, blocker escalation | ✅ Always |
| 5 | Lifecycle | Agent add/delete/re-add, orphaned scores, implicit domain creation, name reuse | ✅ Always |
| 6 | Concurrency | Skip unless project explicitly uses threads/async — note "not applicable" | ⚠️ Rarely |
| 7 | Error Handling | stdin EOF mid-operation, partial I/O failures, validation path inconsistency | ✅ Usually |
| 8 | RCE | Skip — note "out of scope" | ❌ Never |

Generate 3–8 scenarios per applicable dimension. Prioritize severity over
coverage — one CRITICAL finding beats 10 PASSes.

#### Standard Results JSON Format

```json
{
  "project": "Project Name",
  "mode": "live-sandbox",
  "dimensions_tested": [1, 2, 3, 4, 5],
  "dimensions_not_tested": [6, 8],
  "total_scenarios": 24,
  "summary": {
    "CRITICAL": 2, "HIGH": 4, "MEDIUM": 8, "LOW": 3, "PASS": 7
  },
  "findings": [
    {
      "severity": "CRITICAL",
      "dimension": 1,
      "title": "Short descriptive title",
      "flow": "The exact commands run (copy-paste ready)",
      "observed": "What actually happened",
      "expected": "What should have happened instead",
      "remediation": "Concrete fix instruction — the 'boundary note'",
      "theme": "score-integrity"
    }
  ]
}
```

### Phase 3 — Execute

1. **Set up sandbox**: Create a temp dir, set up venv, pip install the project
   in editable mode.
2. **Run**: `python wde_assessment.py` — saves `wde_results.json` on completion.
3. **Verify**: Check that the temp dir is clean and results are valid JSON.

If the script crashes mid-run, fix the crash (it's usually a test-harness
issue) and re-run. Do NOT stop on the first failure — collect as many
findings as possible.

If no sandbox can be constructed, switch to **dry-run mode**:
1. Trace the code path analytically
2. Report with confidence: **certain** / **likely** / **uncertain**
3. Mark as "recommended manual test"

### Phase 4 — Render & Open Report

```bash
python scripts/render_report.py \
  --input wde_results.json \
  --output wde_report.html \
  --title "Project Name"
open wde_report.html || true
```

The renderer (`scripts/render_report.py`) is a standalone stdlib script that:
- Reads results JSON (stdin or `--input`)
- Auto-generates ELI5 explanations from each finding's fields
- Renders a dark-themed HTML with severity badges, finding cards, theme
  groupings, and severity filter tabs
- Outputs agent-to-agent frontmatter to stderr
- Discovers config from `scripts/wde_config.json` (overridable via `--config`)

### Phase 5 — Deliver Results

```
Worst Day Ever complete for [Project].

Ran [N] scenarios across [dims]. Found [X] CRITICAL, [Y] HIGH, [Z] MEDIUM.

HTML report: [link to wde_report.html]
Frontmatter ready for next agent handoff.
```

Save the session summary with full findings data so the next agent can pick up.

## Scoring Guidelines

- **Critical**: Security breach, data loss, unhandled exception that propagates as 500
- **High**: Silent data corruption, auth bypass that doesn't require privilege to exploit, cascade failures
- **Medium**: Confusing UX, uninformative error messages, behavior that surprises but doesn't break
- **Low**: Error handling works but the error message is poor, edge case that's technically handled but ugly
- **Pass**: Correct response with informative message, graceful degradation, proper circuit-breaking

## ELI5 Auto-Generation

The render script **auto-generates** ELI5 explanations from each finding's data
fields. No hand-authored content or keyword-matched templates are needed.

Each finding with `flow`, `observed`, `expected`, and `remediation` fields gets
a 4-paragraph ELI5 block:

1. **What you could do** — restates `flow` in plain English
2. **What the system does** — highlights the gap between `observed` and `expected`
3. **The reason** — explains why an engineer might make this mistake, expands glossary terms
4. **The fix** — turns `remediation` into actionable guidance

### Glossary Expansion

ELI5 blocks expand glossary terms found in the finding text. Glossary entries
are defined in `scripts/wde_config.json` and cover domain-specific concepts
like EMA, fixed point, hysteresis, and IEEE 754 semantics. The glossary can
be extended without touching the renderer code.

### Theme Integration

Findings with a `theme` field get their theme's icon and description rendered
next to the finding card. Themes are defined in `scripts/wde_config.json`.
If no `theme` field is present, the renderer falls back to keyword matching
against the theme's `match_keywords` list.

## Memory Boundary Notes

Every remediation field IS a boundary note — it tells future engineers:
- **Where** the boundary is (which function, which check)
- **What** guard to install (validation, warning, confirmation)
- **Why** it matters (what happens without it)

Use the phrase "I'll make a note at this boundary" when presenting findings
conversationally. The HTML report renders this as a "boundary note" block.

## Bundled Resources

- `scripts/render_report.py` — Standalone report renderer (stdlib only).
  Usage: `python scripts/render_report.py --input <json> --output <html> [--title "Title"] [--config <path>]`
- `scripts/wde_config.json` — Config file with severity order/colors/labels,
  dimension names, themes with keyword matchers, and ELI5 glossary terms.
  Auto-discovered by the renderer; overridable via `--config`.
- `assets/tui.html` — Terminal-style splash animation (see TUI section below).

## Agent Handoff Frontmatter

The render script outputs YAML frontmatter to stderr containing:

```yaml
handoff:
  project: Project Name
  assessment_type: worst-day-ever
  date: "{{DATE}}"
  total_scenarios: 24
  severity_counts:
    CRITICAL: 2
    HIGH: 4
  critical_findings:
    - NaN/Inf bypass (Dim 1): Guard against NaN/Inf in gate comparison
  next_recommendation: Fix CRITICAL items first, then HIGH items.
```

This frontmatter is embedded in the HTML report for agent-to-agent handoff.
The next agent can parse it to understand assessment state without re-reading
the full report.

## TUI Splash Screen

A self-contained terminal-style animation is available at `assets/tui.html`.
Open it in a browser to show users a visual "worst day ever" splash while
the skill runs.

**Design:** Built using the [Linear](https://linear.app) design system.
Near-black `#08090a` canvas, Inter + JetBrains Mono typography, luminance-based
elevation, hostile-red accents for critical findings.

**Features:**
- Large "WORST DAY EVER" title (42px Inter weight-600)
- ASCII frown face with JS-driven eye blink (`◉`→─, every 2.5–5.5s)
- 3×2 status grid: uptime, critical count, resilience score, dimensions, mode, last event
- Auto-scrolling event stream with timestamp + OK/WARN/CRITICAL tagged entries
- Glitch animation on critical findings
- Blinking cursor on prompt line

**Usage:** open `assets/tui.html` from a terminal, or open it directly from your file manager.
The browser blocks `file://` URLs for local assets — serve with
`python3 -m http.server` in the skill directory if needed.

## When to Use This Skill

- Pre-launch review of a new project
- After major refactor, before merge
- When onboarding to a new codebase and wanting to understand failure modes
- After a production incident ("how could this have happened? what else?")
- Quarterly resilience review of a live system

## Relationship to Other Techniques

- **Root-cause analysis** — use when a finding reproduces in production
- **Test-driven development** — recommended tests become TDD cases for the next session
- **Architecture review** — if systemic patterns emerge, hand off architectural recommendations
- **Refactoring plans** — if findings require significant code changes, produce an incremental plan

## Pitfalls

1. **"It worked in dev" is not proof.** The sandbox is deliberately harder than
   dev. If sandbox passes, the real system may still fail.

2. **Don't confuse coverage with resilience.** A system that handles 6 out of 7
   dependency failures is not "mostly resilient" — the 7th is the one that
   costs you at 3 AM.

3. **Null bytes in strings are not dead.** A `\x00` mid-string crashes C-backed
   libraries downstream.

4. **Brownfield mining can leak PII if not careful.** Always sanitize logs
   *before* analysis. Strip emails, JWTs, credit card numbers.

5. **Dry-run is a lower bound.** "The code path looks safe" does not mean it's
   safe. Use dry-run to prioritize what to actually run.

6. **Don't skip Dimension 1 because "we validate inputs."** The validator was
   written by someone who didn't anticipate Unicode RTL overrides in a username
   field.

7. **If you can't create a test user safely, skip AuthN/Z** rather than using a
   real user's session. Report why it was skipped.

8. **Browser tools block `file://` URLs.** Serve local assets with
   `python3 -m http.server <port>` and navigate to `http://localhost:<port>/`.
