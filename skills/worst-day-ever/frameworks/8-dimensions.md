# 8 Attack Dimension Reference

The worst-day-ever framework tests systems across 8 orthogonal dimensions. Each dimension should produce 3–5 scenarios of escalating severity.

| Dimension | What it tests | Key scenarios |
|---|---|---|
| **1. Input Boundary** | Max-length strings, null bytes, Unicode RTL overrides, type confusion, deep nesting, NaN/Infinity | Violate every type contract |
| **2. State Machine Torture** | Double-submit, back-button resurrection, out-of-order transitions, concurrent mutations | Break every state transition invariant |
| **3. Temporal/Timing** | Expired tokens, clock skew, counter races, connection pool depletion | Exploit every time-dependent assumption |
| **4. AuthN/Z Shadow** | Horizontal/vertical escalation, scope mismatch, cross-tenant access | Access every unauthorized path |
| **5. Data Integrity Cascade** | Delete referenced entities, encoding roundtrips, pagination edge cases, constraint bypass | Corrupt every data relationship |
| **6. Concurrent Load** | Thundering herd, cache death spiral, write skew, lock escalation | Overload every shared resource |
| **7. External Dependency Failure** | Payment gateway down, email timeout, DNS failure, malformed third-party responses | Fail every external call |
| **8. Brownfield Mining** | Parse production logs for worst errors, replay with variations | Learn from real failures |

## Severity Classification

| Level | Definition |
|---|---|
| **Critical** | Security breach, data loss, unhandled exception that propagates as 500 |
| **High** | Silent data corruption, auth bypass that doesn't require privilege |
| **Medium** | Confusing UX, uninformative error, behavior that surprises but doesn't break |
| **Low** | Error handled but message is poor, edge case with no real impact |
| **Pass** | Correct response with informative message, graceful degradation |

## Greenfield vs Brownfield

- **Greenfield** (no traffic): Derive attack surface from API specs, type defs, route definitions
- **Brownfield** (live): Mine sanitized production logs for real error patterns, then replay variations

**Brownfield requires Dimension 8. Greenfield skips it.**

## Sandbox Requirements

| Project Type | Sandbox Method |
|---|---|
| Python/Flask/Django | In-memory SQLite + HTTP stubs + mocks |
| Node/Express | better-sqlite3 in-memory + nock + sinon |
| Rails | Test-mode DB + WebMock |
| Go | `testing` package + httptest |
| Static site | Playwright in ephemeral container |
| CLI tool | tmpfs + stubbed stdin/stdout |

If no sandbox can be constructed, use **dry-run mode** (analytical code tracing only).
