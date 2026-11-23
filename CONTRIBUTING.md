# Contributing to Seoran

A useful first contribution can be a Windows build report, a keyboard walkthrough, a documentation correction or a focused code change. English and 中文 are both welcome. If you have an hour and no development setup, the pinned issue lists ways to help. [Start here](https://github.com/INIT-collective/Project_Seoran/issues/1)

## Pick up a task

1. Open the unassigned ready queue. Read the first step and the completion criteria. Comment “I can take this”, what you plan to try, and whether you want a partner. A maintainer assigns the first agreed slice, removes `ready` and adds `in progress`. [Ready issues](https://github.com/INIT-collective/Project_Seoran/issues?q=is%3Aissue%20is%3Aopen%20label%3Aready%20no%3Aassignee)
2. For a test or research task, post the requested observations in the issue. Include the commit and environment so a second person can check them. A maintainer may close a completed report directly and open a linked fix.
3. For new ideas, search existing issues first. Describe one user, the problem they hit, and an observable result. A maintainer can narrow it, mark it `backlog`, or mark it `ready` when someone can pick it up. Engine, privacy-default and provider decisions use the proposal template. [Open an issue](https://github.com/INIT-collective/Project_Seoran/issues/new/choose)

## Make a change

If you have write access, clone the shared repository. Otherwise use GitHub's **Fork** button, clone your fork, and add the shared repository as `upstream` once.

```sh
git remote add upstream https://github.com/INIT-collective/Project_Seoran.git
```

Create a short-lived branch for a claimed issue. Here a contributor has claimed issue 8 and named the branch `feat/8-find-in-page`. With a fork, replace `origin` with `upstream` in the first two commands. The push still goes to `origin`.

```sh
git fetch origin
git switch -c feat/8-find-in-page origin/main
# Make the change, run the relevant checks and commit the intended files.
git push -u origin HEAD
```

Open a PR against `INIT-collective/Project_Seoran:main`. Put `Closes #8` in the description when the PR completes the issue. Use `Refs #8` for partial work. Make a draft when an early design choice needs feedback. New branches start when work starts so nobody has to maintain an empty branch. The repository-level license decision is open. Record licensing questions about new or imported material on the decision issue. [License discussion](https://github.com/INIT-collective/Project_Seoran/issues/4)

## Verify and request review

| Change | Evidence in the PR |
| --- | --- |
| Rust or engine | Run `cargo build --locked -p seoran` and `cargo test --locked --workspace` on Windows. Include the test result and the affected user journey. |
| Shield or filter behavior | Include the existing Node scriptlet check, or link the Windows CI run that executes it. Describe expected ordinary page behavior with the change. |
| UI or keyboard | Give exact steps with expected focus, one screenshot or short recording if helpful, and test light/dark when appearance changes. Crop out personal data. |
| Documentation or research | Follow the edited instructions or check claims against the named source. Link the result or decision. |

When your PR is ready, request a reviewer with write access other than the author. The repository administrator, @CmsChase, can review contributor PRs. Anyone can test, comment or pair. The reviewer checks the issue criteria, evidence, and any change to stored data, privacy defaults or update responsibilities. Use **Request changes** for a blocker, **Comment** for optional suggestions and **Approve** when the criteria are met.

The author addresses blocking feedback, pushes fixes and requests another look. For ordinary contributors, GitHub requires one independent approval, resolved review conversations and both CodeQL language checks on `main`. New reviewable changes dismiss stale approvals. For an administrator-authored PR, the administrator can use GitHub's branch-protection override and make the final merge decision. CI still runs and its results are available before merging. Check the Windows job as well when it is available. After a squash merge, confirm the linked issue closed and remove the merged topic branch. [Windows CI rollout](https://github.com/INIT-collective/Project_Seoran/issues/3)

## Keep a small queue moving

- A maintainer checks in when available. Three ready choices give people a way in. Pull a candidate from the backlog after a slot opens. Aim for at most two implementation tasks in flight across the team.
- Focus on one owned task at a time. To hand it back, post what you tried, link the branch or draft, and say what remains. The maintainer clears the assignee and restores `ready`.
- If a claim has been quiet for two weeks, a maintainer can ask once whether help would be useful. After another week without a reply, reopen the slot and link the existing work. A quiet week carries no personal obligation.
- If review stalls, ask once in the PR and invite another eligible reviewer if one is available. Keep the branch open until someone reviews it. A second tester can still contribute evidence in the meantime.
- For a contested decision, the proposer records options, costs, a recommendation and a revisit condition. A maintainer invites affected contributors and records the outcome in the issue. If nobody can own the ongoing work, park it in the backlog. An implementation PR follows an accepted decision.

For a possible vulnerability, report it privately to the maintainers through GitHub. Keep private browsing data and credentials out of public reports. [Report a vulnerability](https://github.com/INIT-collective/Project_Seoran/security/advisories/new)
