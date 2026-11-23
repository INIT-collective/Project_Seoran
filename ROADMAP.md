# Seoran product roadmap

## What success looks like

A volunteer is answering a mod compatibility question on Discord. They have a private log containing a local username, two versions of public documentation and a GitHub discussion. In Seoran, they save the useful passages in a named case, resume it tomorrow and give the person asking a short answer with links to the evidence. The recipient can verify that answer in any browser.

In the proposed finished experience, the helper can ask an on-device model about the log. When they switch to an API model for a harder question, Seoran builds a fresh request from a new question and the public passages they approve. The preview names the provider and shows the exact content. The private log, its URL, derived answers and earlier chat stay on-device. If the local model is unavailable, the helper can still collect evidence and write the reply. The community can share a setup with public references, keyboard preferences and a reply template. Credentials, case notes and browsing history stay with their owner.

The proposed first audience is mod authors, maintainers and volunteer support helpers who routinely work with public and private sources. Ask them to try Seoran for one real question before asking them to change their default browser. Repeated use for that job is the first sign that the project deserves a larger release.

## Starting point

The current Windows prototype has a Rust shell using Tao, Wry and WebView2, tabs and session restore, bookmarks, a command palette, themes, Shield and built-in DevTools. The case notebook, model handoff and community beta below are planned work. [Current implementation](README.md) · [Product brief](PRODUCT.md)

## Milestones and release decisions

| Stage | Deliverable | Evidence to finish |
| --- | --- | --- |
| M0 · Contributor setup | A contribution guide, Windows build and tests on PRs, and a project license with notices. | A volunteer can claim a task, the guide and checks are merged, and the authors record their license choice. [M0 tracker](https://github.com/INIT-collective/Project_Seoran/milestone/1) |
| M1 · Windows preview | A second person can run Seoran and finish an ordinary session. In-page find is the first feature candidate. Tester reports can change the choice. | Record the tested commit and Windows/runtime versions. Exercise keyboard navigation, bookmarks, restart and Shield. Review the data inventory and trust boundaries, then record go or hold. [M1 tracker](https://github.com/INIT-collective/Project_Seoran/milestone/2) |
| M2 · Source notes and handoff | A local case notebook, source-linked excerpts and a reviewed Markdown export usable with AI off. | Reopen a case after restart. A peer using their usual browser verifies two citations. A local-only test excerpt and its metadata are absent from the draft. Compare the effort with the helper's current workflow. [M2 tracker](https://github.com/INIT-collective/Project_Seoran/milestone/3) |
| M3 · Local and API model choice | One on-device path and one opt-in remote API. Build a fresh request when switching from private chat to a remote provider. | A controlled endpoint records only the confirmed question and approved excerpts. Private text, source URLs, derived answers and chat stay out. Test off, cancel, offline and local-model failure paths. [M3 tracker](https://github.com/INIT-collective/Project_Seoran/milestone/4) |
| M4 · Community beta decision | A reusable setup for recurring cases and a small opt-in pilot. | Seek 3–5 helpers, at least two voluntary repeat uses within a week, and one independently verified handoff. A beta go also needs passing privacy tests and an owner who rehearsed an update and rollback. Record go, revise or pause, including any shortfall in evidence. [M4 tracker](https://github.com/INIT-collective/Project_Seoran/milestone/5) |

A maintainer can start interviews during the Windows preview. Ask a participant to recall their last real support question and walk through their usual tools. Use public or sanitized material. Record copy-and-paste steps, time to a verifiable reply, and where they expected private material to stay. Build one implementation slice at a time. The live ready queue is the source of current openings. [Baseline study](https://github.com/INIT-collective/Project_Seoran/issues/19) · [Start here](https://github.com/INIT-collective/Project_Seoran/issues/1)

During the pilot, repeat a comparable task in Seoran. Before an API call, ask the helper to identify the recipient and what it will receive. Follow up after a week to learn whether they chose Seoran for another case without a reminder to use it. Voluntary reports are sufficient. The sample and repeat-use targets guide a small team's decision and are not adoption forecasts. If the benefit is confined to clipping and citations, compare an extension before taking on more browser maintenance.

## Longer-term options

| Direction | When to revisit it |
| --- | --- |
| Everyday features and customization | If a tester identifies a blocker, choose one of in-page find, download status, bookmark search or toolbar density. Compare keyboard access, restart behavior and the user's existing profile. [Find](https://github.com/INIT-collective/Project_Seoran/issues/8) · [Downloads](https://github.com/INIT-collective/Project_Seoran/issues/9) · [Bookmarks](https://github.com/INIT-collective/Project_Seoran/issues/10) · [Compact toolbar](https://github.com/INIT-collective/Project_Seoran/issues/11) |
| Privacy, security and performance | Map what clearing site data removes and who updates filters and the engine. Treat isolated browsing profiles as a separate capability that needs restart and storage tests. For performance comparisons, record cold starts and the same ten-tab session on a named machine. [Data inventory](https://github.com/INIT-collective/Project_Seoran/issues/7) · [Trust boundaries](https://github.com/INIT-collective/Project_Seoran/issues/15) |
| AI actions | Once reading and handoff prove useful, test grouping tabs or drafting a reply. Define the permission boundary and confirmation for each action. The beta workflow ends with a human-reviewed draft. [Selected-passage design](https://github.com/INIT-collective/Project_Seoran/issues/13) |
| Markets/News | A volunteer who follows a topic daily can compare a separate source workspace with their existing bookmark routine. Record the advantage and the cost of a live feed before selecting it for a milestone. [Mode proposal](https://github.com/INIT-collective/Project_Seoran/issues/14) |
| Tor | Establish the user need and privacy expectations first. Consider an explicit handoff to the official Tor Browser. Its project cautions that routing another browser through Tor can expose identifying information. [Tor decision](https://github.com/INIT-collective/Project_Seoran/issues/26) · [Tor Project guidance](https://support.torproject.org/tor-browser/security/using-tor-with-other-browsers/) |

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

The engine choice, additional platforms and optional modes depend on a demonstrated need and someone willing to maintain them. The milestones have exit criteria and no fixed calendar deadlines.
