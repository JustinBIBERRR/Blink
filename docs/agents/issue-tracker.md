# Issue tracker

Track Blink work and specs in [GitHub Issues](https://github.com/JustinBIBERRR/Blink/issues). Use `gh` from the repository checkout so it resolves the GitHub remote.

## Working with issues

- Publish a spec or ticket with `gh issue create --title "..." --body-file <file>`.
- Read a ticket and its discussion with `gh issue view <number> --comments`. Fetch structured details with `gh issue view <number> --json number,title,state,body,labels,comments`.
- List work with `gh issue list`, adding `--state`, `--label`, or `--json` as the task requires.
- Comment with `gh issue comment <number> --body-file <file>`.
- Apply or remove a label with `gh issue edit <number> --add-label <label>` or `gh issue edit <number> --remove-label <label>`.
- Close resolved work with `gh issue close <number>`.

## Pull requests as a triage surface

**PRs as a request surface: no.**

If this flag is set to `yes`, apply the same triage states to external pull requests. List open pull requests with `gh api --paginate 'repos/JustinBIBERRR/Blink/pulls?state=open&per_page=100'` and keep those whose `author_association` is `CONTRIBUTOR`, `FIRST_TIME_CONTRIBUTOR`, or `NONE`. Review with `gh pr view <number> --comments` and `gh pr diff <number>`. Use the `gh pr` equivalents to comment, label, or close. GitHub shares the number space between issues and PRs. Resolve an ambiguous number with `gh pr view` and then `gh issue view`.

## Wayfinding operations

- **Map.** Create one issue labelled `wayfinder:map` with the Notes, Decisions-so-far, and Fog sections.
- **Child ticket.** Create a GitHub sub-issue via `gh api` and apply `wayfinder:<type>`, where the type is `research`, `prototype`, `grilling`, or `task`. If sub-issues are unavailable, add the issue to the map's task list and start its body with `Part of #<map>`.
- **Dependencies.** Add a blocker with `gh api --method POST repos/JustinBIBERRR/Blink/issues/<child>/dependencies/blocked_by -F issue_id=<blocker-db-id>`. Resolve the blocker's numeric database ID with `gh api repos/JustinBIBERRR/Blink/issues/<blocker> --jq .id`. Use `issue_dependencies_summary.blocked_by` to check for open blockers. If native dependencies are unavailable, put `Blocked by: #<number>` at the top of the child ticket and check whether the listed issues are closed.
- **Frontier.** Read the map's open children in map order and take the first unassigned ticket with every blocker closed.
- **Claim.** Assign the ticket to yourself with `gh issue edit <number> --add-assignee @me` as the session's first write.
- **Resolve.** Comment with the outcome, close the child ticket, and add a short decision summary and link to the map's Decisions-so-far section.
