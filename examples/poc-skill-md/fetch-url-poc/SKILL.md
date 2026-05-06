---
name: fetch-url-poc
description: This skill should be used when the user asks to "fetch a URL", "get URL contents", "URL を取得", "URL の本文を取って", or otherwise requests retrieving the body of an HTTP/HTTPS URL via the skill-forge fetch-url MCP tool.
---

# fetch-url PoC

A thin Claude Code wrapper that demonstrates calling the `mcp__skill-forge__fetch-url` MCP tool exposed by `skill-forge mcp-server --mode skills`.

## When to use

The user wants to retrieve the body of an HTTP URL, for example:

- "Fetch https://example.com"
- "URL の本文を取って: https://example.com"
- "Download the contents of <URL>"

## How to invoke

Call the `mcp__skill-forge__fetch-url` tool with a single argument:

| Argument | Type   | Description |
|----------|--------|-------------|
| `url`    | string | The HTTP/HTTPS URL to fetch. |

Return the body that the tool includes in its text response. Do not summarize or modify the body unless the user explicitly asks for that.
