# wde-mcp-server

An MCP server that exposes the **Worst Day Ever** skill over the Model Context Protocol.

Built on [`rmcp`](https://github.com/modelcontextprotocol/rust-sdk) with the SEP-2640 Skills extension.

## What it does

Worst Day Ever is an 8-dimension assessment tool. It scores a situation across 8 axes (authN/Z, data integrity, frontend, deployment, etc.) and produces a prioritized severity report.

This server exposes it as:

- A **skill** — via the `io.modelcontextprotocol/skills` extension
- A **tool** — `worst_day_ever_assess` callable by any MCP client
- **Resources** — the 8 dimension files readable individually

## Run

```bash
# stdio transport (for MCP Inspector / stdio clients)
cargo run --bin wde-mcp-stdio -- ./skills

# HTTP transport (streamable HTTP)
cargo run --bin wde-mcp-http -- ./skills
```

HTTP mode listens on `127.0.0.1:3001/mcp`.

## Test

```bash
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

## Repo layout

```
src/
  main.rs           — stdio binary
  bin/http_server.rs — HTTP binary (axum)
  lib.rs            — server implementation
skills/
  wde-skill.md      — skill manifest
  worst-day-ever/   — skill content (8 dimensions + SKILL.md)
```
