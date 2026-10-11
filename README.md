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
cargo run --bin wde-mcp-stdio

# HTTP transport (streamable HTTP)
cargo run --bin wde-mcp-http
```

Both default to `./skills` as the skill root. HTTP mode listens on `127.0.0.1:3001/mcp`.

## Connect a client

### Python (zero-config)

```python
import asyncio
from mcp import ClientSession, StdioServerParameters
from mcp.client.stdio import stdio_client

async def main():
    params = StdioServerParameters(
        command="cargo", args=["run", "--bin", "wde-mcp-stdio"],
        cwd="/path/to/wde-mcp-server",
    )
    async with stdio_client(params) as (read, write):
        async with ClientSession(read, write) as session:
            await session.initialize()
            # List available skills
            skills = await session.list_skills()
            print(skills)
            # Call the assessment tool
            result = await session.call_tool(
                "worst_day_ever_assess",
                {"project_path": "/path/to/target/project"},
            )
            print(result)

asyncio.run(main())
```

### Rust

```rust
use rmcp::ServiceExt;
use wde_mcp_server::WdeServer;

let server = WdeServer::from_root("./skills")?;
let transport = rmcp::transport::stdio();
let service = server.serve(transport).await?;
service.waiting().await?;
```

### HTTP (any client)

```bash
curl -X POST http://127.0.0.1:3001/mcp \
  -H "Content-Type: application/json" \
  -H "Accept: application/json, text/event-stream" \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2026-07-28","capabilities":{},"clientInfo":{"name":"test","version":"1"}}}'
```

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
tests/
  agent_drive.rs    — regression tests for the agent-drive path
skills/
  worst-day-ever/   — skill content (8 dimensions + SKILL.md)
```
