# Seoran product roadmap

Seoran's first milestone for a user is an ordinary browsing session they choose to repeat tomorrow. Imagine a student researching across several tabs on a Windows laptop. They find a passage, bookmark the page, see what Shield blocked and come back after a restart. A preview tester should be able to do that with the keyboard, tell us where they got stuck and explain where the session was saved.

The repository has a working Rust shell using Tao, Wry and WebView2 on Windows. Tabs, session restore, omnibox suggestions, bookmarks, local settings, six frame colours, Shield and built-in DevTools are already present. These are the starting point for a community preview. The priorities below are proposals the team can change with evidence. [Current implementation](README.md) · [Product brief](PRODUCT.md)

## What to build next

| Direction | Smallest useful experiment | Evidence for the next decision |
| --- | --- | --- |
| Everyday browsing | A tester tries the research session above. Choose one of in-page find, download status or a searchable bookmark list based on the first point of friction. | A second person completes the task with a keyboard and reproduces the result after restart. [Find](https://github.com/INIT-collective/Project_Seoran/issues/8) · [Downloads](https://github.com/INIT-collective/Project_Seoran/issues/9) · [Bookmarks](https://github.com/INIT-collective/Project_Seoran/issues/10) |
| Customization | Try one compact toolbar setting on a small laptop. Reuse the current themes and the user's chosen frame colour. | Compare available page space, readability, focus and clipped menus at two window sizes. If people need different workflows, explore saved layouts next. [Compact toolbar](https://github.com/INIT-collective/Project_Seoran/issues/11) |
| Privacy and security | Trace one visit from the page to history, engine site data and Shield. Show what clearing history actually removes and who updates the engine and filters. | Publish the data inventory and review the boundary around native actions. A private window is a candidate once someone can test isolation across close and restart. [Data inventory](https://github.com/INIT-collective/Project_Seoran/issues/7) · [Trust boundaries](https://github.com/INIT-collective/Project_Seoran/issues/15) |
| Performance | Collect three cold starts and one ten-tab session on the same test laptop. Record commit, runtime, page fixture and memory before and after tabs sleep. | Use the measurements to judge regressions. Publish the method with the numbers. Volunteer reports keep this possible without adding telemetry. |
| AI integration | Select a passage, preview the exact text and destination, then ask for a summary. Prototype with a mock provider before choosing local inference or an optional remote service. | Show zero requests while off, a cancel path, and only the approved selection reaching the mock. Tab actions and email drafting need separate decisions about permissions and confirmation. [AI proposal](https://github.com/INIT-collective/Project_Seoran/issues/13) |
| Markets/News mode | Reopen a small collection of sources in a dedicated workspace and switch back without losing the original tabs. Walk through it with someone who follows a topic daily. | Ask whether it improves their existing bookmark or tab routine. Learn which source and account requirements would come with a live feed. [Mode proposal](https://github.com/INIT-collective/Project_Seoran/issues/14) |

## Who would maintain the engine?

The shell and the rendering engine are separate investments. The proposal is to keep the first Windows preview on its working WebView2 backend. CEF is the first migration candidate to measure. A backend decision also affects input, child-window clipping, profile formats, Shield and DevTools. Contributors should check those paths on a real build before promising another platform.

| Option | What the team gains | Work the team would own |
| --- | --- | --- |
| Keep WebView2 for the preview | The current shell runs on it. Microsoft's Evergreen runtime has an upstream update path. | Detect a missing runtime, test updates and Windows-specific behavior, document runtime and profile assumptions. [Microsoft runtime documentation](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/evergreen-vs-fixed-version) |
| Embed CEF | A Chromium embedding framework to try behind the existing shell, including on another desktop OS. | Integrate input, windowing, Shield and DevTools, package the runtime and rehearse an upstream upgrade. [CEF project](https://github.com/chromiumembedded/cef) |
| Fork Chromium | Direct access to browser and engine behavior that embedding APIs might not expose. | Name the capability that requires a fork, then budget builds, upstream merges, security releases and platform testing. [Chromium build guides](https://www.chromium.org/developers/how-tos/get-the-code/) |
| Embed Servo | A Rust engine with an embedding API offers a separate compatibility experiment. | Run the team's same page fixtures and measure missing APIs, rendering differences and integration work. [Servo project](https://servo.org/) |
| Write an engine | Contributors can study a tightly scoped renderer and make original design choices. | Start with a research demo and a modest HTML/CSS target. A general-purpose engine would need owners for layout, scripting, networking, isolation, compatibility tests and long-term fixes. |

For a repeatable comparison, use one machine and the same local text, form and resource-filter fixtures. Record navigation, focus, history, blocking, cold start, package size and the steps to update the runtime. A macOS or Linux claim needs a volunteer who builds and tests on that OS. Record the measured gaps, ongoing owner and a keep, switch or revisit decision. [Engine decision](https://github.com/INIT-collective/Project_Seoran/issues/12)

## Milestones and release decisions

| Milestone | What the community can deliver | How it finishes |
| --- | --- | --- |
| M0 · Contributor setup | Publish the contribution path, run browser checks on Windows and get the authors' license decision. | A new volunteer can claim a task. A second maintainer can review a linked PR with working checks. The agreed license and notices are recorded. [M0 issues](https://github.com/INIT-collective/Project_Seoran/milestone/1) |
| M1 · Windows preview | Start with a clean build report, keyboard walkthrough and data inventory. Then pick one everyday improvement and review the native trust boundary. Other feature candidates can move to a later milestone. | On a second machine, test navigation, tabs, bookmarks, session restore, Shield and the chosen improvement. Link the tested commit, environment, known limitations and release decision. [Preview rehearsal](https://github.com/INIT-collective/Project_Seoran/issues/16) |
| M2 · Product experiments | Pick one engine, AI or workspace question when someone volunteers to own it. | Publish the smallest prototype or comparison, a tester's observation, and an explicit decision to proceed, revise or park it. Spin out implementation only when someone can maintain it. [M2 issues](https://github.com/INIT-collective/Project_Seoran/milestone/3) |

Milestones have exit criteria and no calendar deadlines. The pinned issue is the place to recruit for active work and propose a different order. [Start here](https://github.com/INIT-collective/Project_Seoran/issues/1)
