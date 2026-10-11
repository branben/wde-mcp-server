# Safeguards

Non-negotiable rules for every worst-day-ever run.

1. **No real data** — all inputs synthesized. Real schemas used only for type/format inference
2. **No secrets** — if the system requires auth, create a test token. Never read `.env` or keyrings
3. **Sandboxed execution** — run in isolated subprocess / DB fake / HTTP stub. No writes to shared state
4. **No external calls** — if the system hits external services (payment gateway, email API), stub them with failure modes
5. **Bounded duration** — each flow runs max 30 seconds. If it hangs, kill it and report "hang detected"
6. **No mutation side effects** — if the system writes to a screen/file, capture the write. If it writes to a DB, roll back or use a throwaway

## When safeguards cannot be met

Abort and tell the user which safeguard failed and why. Do not proceed with partial safeguards.
