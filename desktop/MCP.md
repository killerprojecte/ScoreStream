# Desktop MCP service

Open **MCP** in the document toolbar, choose a numeric listen IP and port, and select **Start service**. The default is `127.0.0.1:3210`. Copy the displayed SSE URL into an MCP client that supports the legacy **HTTP + SSE** transport. This is not the newer Streamable HTTP transport.

- **Save settings** saves only the listen IP and port, without opening a listener.
- **Start service** validates and saves those settings, then opens the listener. Binding failures remain visible and can be retried after changing the port or releasing it.
- **Stop service** closes active SSE connections and releases the listener. Stop before changing settings.
- The service always starts **off** when the app opens. The native app shuts it down when exiting.
- The optional key is masked, is held only for the running session, and is never included in persisted preferences, status responses, copied URLs, or logs. It must be entered again after stopping or restarting the app. Closing the dialog clears any unsubmitted key. There is no existing OS credential store integration, so no plaintext-secret persistence fallback is used.

If configured, supply `Authorization: Bearer <key>` on **both** the SSE GET and message POST. A client's plain browser `EventSource` API without custom authorization headers is insufficient for keyed connections. Never put the key in a URL. The SSE endpoint event supplies the session-specific POST endpoint; clients should use that endpoint rather than inventing one.

## Network boundary

Keep `127.0.0.1` or `::1` for local clients. Any non-loopback listener requires a key and displays a network-access warning. Use a long, random key. HTTP does not encrypt either the key or score content, so remote use should be limited to trusted networks or a separately configured secure tunnel. The app does not change firewall, network, or operating-system security settings.

For a wildcard bind (`0.0.0.0` or `::`), replace the wildcard host in the displayed URL with this computer's reachable network IP when configuring another device. Numeric-IP host validation and strict Origin checks protect against arbitrary web origins; this is not an unrestricted browser/CORS endpoint. No non-loopback listener is opened by the automated desktop tests.

## One workspace, read-only MCP

The native desktop owns one `Arc<Mutex<Workspace>>`, shared directly with the MCP handler. There is no subprocess or secondary score state. `scorestream://workspace/current` reads the committed source, typed document and revision through that same lock. Editor Apply, notation edits, undo and redo become visible on the client's next resource read. Live uncommitted editor text is never returned.

Only format discovery and committed-workspace reading are advertised. MCP score-edit tools and resource subscriptions are not implemented. Since MCP cannot mutate the workspace, it cannot replace the editor's private draft or create an external revision change that needs merging. Clients should re-read the resource when they need a newer revision; no resource-change notifications are advertised or promised.

## Storage and verification

Preferences are written atomically to `mcp-service.json` in Tauri's application configuration directory. The schema contains only `bind` and `port`. Invalid, oversized or unexpected-field files fall back to localhost defaults, with a visible diagnostic. Saving a network address never enables automatic startup.

Frontend checks:

```sh
cd desktop
npm test -- --maxWorkers=1
npm run build
```

Native controller tests (requires the standard Tauri build dependencies):

```sh
cargo test --locked --manifest-path crates/scorestream-desktop/Cargo.toml
cargo clippy --locked --manifest-path crates/scorestream-desktop/Cargo.toml --all-targets -- -D warnings
```

The lifecycle tests bind only loopback addresses, cover occupied ports and retries, stop/restart and port release, and verify that status and saved preferences contain no key. Frontend tests cover repeated clicks, failed start, stale status replies, non-secret persistence, and network-key validation. A successful headless test/build does not establish native-window visual acceptance, clipboard behavior on every WebView, OS shutdown behavior, packaging, or Windows/macOS compatibility.
