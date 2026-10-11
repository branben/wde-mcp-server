# Worst-Day-Ever Report: <project>

**Date:** YYYY-MM-DD
**Mode:** [live-sandbox | dry-run | brownfield-mine]
**Dimensions tested:** N
**Total scenarios:** N

## Summary

| Severity | Count | Description |
|---|---|---|
| Critical | N | Data corruption, security bypass, unhandled crash |
| High | N | Silent wrong answer, orphan records, auth bypass |
| Medium | N | Confusing error, unexpected behavior, no crash |
| Low | N | Cosmetic, uninformative error, edge case with no impact |
| Pass | N | Graceful handling, correct error response |

## Findings

### [CRITICAL] <Title>
- **Dimension:** 1-8
- **Flow:** The pathological user flow, step by step
- **What happened:** System behavior observed
- **Expected:** What should happen
- **Remediation:** Specific fix suggestion, file/line if known

### [HIGH] <Title>
...

### [MEDIUM] <Title>
...

### [LOW] <Title>
...

### PASS <Title>
...

## Resilience Score

**Overall:** X/10

| Dimension | Score |
|---|---|
| Input Boundary | X/10 |
| State Machine | X/10 |
| Temporal/Timing | X/10 |
| AuthN/Z | X/10 |
| Data Integrity | X/10 |
| Concurrency | X/10 |
| External Deps | X/10 |
| Brownfield | X/10 |

## Recommended Tests

Auto-generated test skeletons (pytest/rspec/go-test/etc.) based on critical findings:

```python
def test_input_boundary_null_bytes():
    """Dimension 1: Input Boundary — null byte in username should 400, not 500"""
    response = client.post("/users", json={"username": "test\x00user"})
    assert response.status_code == 400
```
