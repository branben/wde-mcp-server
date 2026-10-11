# Worst-Day-Ever Report: example-api

**Date:** 2026-06-28
**Mode:** dry-run
**Dimensions tested:** 7 (Brownfield skipped — no production logs)
**Total scenarios:** 28

## Summary

| Severity | Count | Description |
|---|---|---|
| Critical | 2 | Auth bypass via scope mismatch; unhandled crash on null byte |
| High | 4 | Orphaned records on user deletion; silent write skew in inventory |
| Medium | 8 | Confusing 500 on expired token; poor pagination edge case handling |
| Low | 6 | Uninformative errors; cosmetic whitespace issues |
| Pass | 8 | Graceful handling of max-length strings out-of-order state transitions |

## Findings

### [CRITICAL] Auth scope mismatch allows order creation with read-only token
- **Dimension:** 4 — AuthN/Z Shadow
- **Flow:** Obtain token with `read:orders` scope → send `POST /orders` with that token
- **What happened:** 201 Created, order persisted
- **Expected:** 403 Forbidden — token lacks `write:orders` scope
- **Remediation:** Add scope enforcement in `auth/middleware.ts:47` — check required scope against token claims before handler dispatch

### [CRITICAL] Null byte in username crashes with unhandled 500
- **Dimension:** 1 — Input Boundary
- **Flow:** `POST /users` with `{"username": "admin\x00test"}`
- **What happened:** 500 Internal Server Error, stack trace leaked in response body
- **Expected:** 400 Bad Request with validation error
- **Remediation:** Add input sanitization before DB layer; wrap user creation in try/catch with sanitized error response

### [HIGH] Deleted user's scheduled jobs continue to execute
- **Dimension:** 5 — Data Integrity Cascade
- **Flow:** Create user → schedule background job → delete user → wait for job trigger
- **What happened:** Job runs with dangling `user_id`, fails silently, no retry
- **Expected:** Job should detect deleted user and skip with audit log
- **Remediation:** Add FK check at job start; consider ON DELETE SET NULL with status flag

...additional findings...

## Resilience Score

**Overall:** 4.5/10

| Dimension | Score |
|---|---|
| Input Boundary | 3/10 |
| State Machine | 6/10 |
| Temporal/Timing | 5/10 |
| AuthN/Z | 2/10 |
| Data Integrity | 3/10 |
| Concurrency | 4/10 |
| External Deps | 7/10 |
| Brownfield | N/A |

## Recommended Tests

```python
def test_auth_scope_enforcement():
    """Dim 4: read-only token must reject write operations"""
    token = create_token(scopes=["read:orders"])
    response = client.post("/orders", headers={"Authorization": f"Bearer {token}"}, json={...})
    assert response.status_code == 403

def test_null_byte_input_rejected():
    """Dim 1: null bytes in username must 400, not 500"""
    response = client.post("/users", json={"username": "test\x00user"})
    assert response.status_code == 400
    assert "stack" not in response.text.lower()
```
