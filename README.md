# Seoran

Seoran is a general-purpose desktop browser from INIT:// focused on deep customization, privacy, security and optional AI. The working Windows prototype uses Rust and WebView2 and includes ad and tracker blocking. The project aims to be open source. The original rights holders are discussing licensing for external contributions and distribution. [License decision](https://github.com/INIT-collective/Project_Seoran/issues/4)

## Join the project

Try one Windows build, test one keyboard interaction, or trace one privacy setting. The pinned welcome issue lists the current openings. [Start here](https://github.com/INIT-collective/Project_Seoran/issues/1) · [Contribution guide](CONTRIBUTING.md) · [Product roadmap](ROADMAP.md)

## Status

Phase 1 base:

- Tabs: open, close, undo close, drag to reorder, middle-click close, session restore, sleeping tabs.
- Address bar with history, bookmark and open-tab suggestions.
- Command palette (`Ctrl K`), shield popover, menu and toasts.
- Bookmarks, a new tab page, settings, and light and dark themes.
- **Seoran Shield**: network blocking, cosmetic hiding and scriptlets (YouTube included), using EasyList, EasyPrivacy and uBlock Origin lists, updated daily.

## Layout

```
app/                  the browser (binary "seoran")
  src/main.rs         browser core: tabs, events, messages between UI and engine
  src/engine.rs       Windows engine integration using WebView2
  src/filters.rs      filter-list download, compile and cache (worker thread)
  src/store.rs        settings, history, bookmarks, session (JSON in the profile)
  src/nav.rs          address-bar input to URL, and URL to display text
  src/pages.rs        built-in seoran:// pages, compiled into the binary
  ui/                 browser UI (chrome.*), new tab, settings, design tokens
crates/shield/        ad and tracker blocker (wraps Brave's adblock-rust)
  lists/              built-in and YouTube filter lists
  scriptlets/         set-constant, json-prune, json-prune-fetch/xhr-response, seoran-yt-skip
tools/                dev tools: window capture, DevTools Protocol driver
```

### How the UI works

The browser UI is a small web page (`seoran://chrome`) that covers the whole window above each tab's page. A native window region clips it to the toolbar, the optional tab rail and any open menu, so menus draw over the live page without hiding, resizing or repainting it. The UI renders state pushed from Rust and sends intents back over IPC.

### Try live tab placement (Windows experiment)

Open the menu and choose **Move tabs to the left**. Choose **Move tabs to the top** to restore the default. Both commands also appear in the command palette. The active page and docked DevTools stay open. Drag tabs along the strip or rail to reorder them.

The menu saves the choice to `%LOCALAPPDATA%\Seoran\layout.json`. To try the declarative path, save `{"tabs":"left"}` or `{"tabs":"top"}` in that file, then run **Reload tab layout from file** in the command palette. A missing file selects top tabs. An invalid file or a failed save returns the running window to top tabs and leaves the pages intact. A successful menu change replaces the file.

For the continuity check, use a disposable Windows profile with top tabs selected. Start the debug build with the loopback DevTools port described in `tools/cdp.mjs`, then run `node tools/layout-continuity.mjs` with Node 22 or later. It opens a loopback page, starts a quiet tone, and checks the same page across 100 menu switches. The native geometry, recovery and session checks run with `cargo test --locked -p seoran windows_tab_lifecycle -- --ignored`.

### Seoran DevTools

`F12`, `Ctrl Shift I` or right-click → **Inspect** opens Seoran's own DevTools, docked at the bottom of the window. Drag its top edge to resize. It talks to the page over the Chrome DevTools Protocol:

- **Elements:** live DOM tree, pick an element in the page (`Ctrl Shift C`), hover to highlight, styles, computed values and box model. `Delete` removes a node, `h` hides it, and `$0` in the console is the selected element.
- **Console:** page logs, errors and exceptions; run JavaScript with history (↑/↓); expandable objects; level and text filters.
- **Network:** every request with status, type, size, timing and a waterfall; headers and response bodies; a filter for requests the shield blocked.
- **Storage:** cookies, local storage and session storage, with delete and clear.
- **Shield:** what the shield did on this page: blocked requests, most-blocked domains, which scriptlets ran, and how many cosmetic rules applied.

**Full DevTools** opens the engine's complete Chromium DevTools in a separate window for deep debugging. They carry Edge branding while the engine is WebView2.

### How YouTube ads are blocked

YouTube serves ads from its own domain, so blocking by domain alone doesn't stop them. Like Brave and uBlock Origin, Seoran injects scriptlets before YouTube's own scripts run. They strip `playerAds`, `adPlacements` and `adSlots` from the player config, from `JSON.parse`, and from `fetch` and XHR responses. A fallback skips any ad that still plays. YouTube changes often, so the filter lists refresh daily. When something breaks, edit `crates/shield/lists/youtube.txt` and the scriptlets.

## Build and run (Windows)

You need stable Rust with the MSVC toolchain, the Visual Studio C++ build tools with a Windows SDK, and the WebView2 Runtime. If the runtime is missing on your Windows installation, install the Evergreen Runtime. [Microsoft's runtime guide](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/evergreen-vs-fixed-version)

```powershell
cargo run -p seoran
```

Release build (smaller and faster):

```powershell
cargo build --release -p seoran
```

Your profile (history, bookmarks, cached filter lists) lives in `%LOCALAPPDATA%\Seoran`.

## Test

```powershell
cargo test --workspace
```

Scriptlet behavior test (Node 18 or newer):

```powershell
cargo run --locked -q -p shield --example dump_script -- 'https://www.youtube.com/watch?v=x' | Out-File -Encoding utf8 "$env:TEMP\yt.js"
if ($LASTEXITCODE -eq 0) { node crates/shield/tests/scriptlets.test.mjs "$env:TEMP\yt.js" }
```

To drive a running Seoran from scripts, start it with a DevTools port:

```powershell
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=9333"; cargo run -p seoran
```

```powershell
node tools/cdp.mjs seoran.chrome "S.shield"
```

## Shortcuts

| Keys | Action |
| --- | --- |
| Ctrl T / Ctrl W | New tab / close tab |
| Ctrl Shift T | Reopen closed tab |
| Ctrl L, Alt D, F6 | Focus address bar |
| Ctrl K | Command palette |
| Ctrl Tab / Ctrl Shift Tab | Next / previous tab |
| Ctrl 1–9 | Go to tab |
| Ctrl D | Bookmark |
| Ctrl , | Settings |
| F12 / Ctrl Shift I | Seoran DevTools |
| Ctrl Shift J | DevTools console |
| Ctrl Shift C | Pick an element to inspect |

## Next steps

The current architecture experiment tests whether tab controls can be rearranged or replaced while the page stays live. A second contributor will be needed to validate the authoring contract. [Roadmap and open decisions](ROADMAP.md)
