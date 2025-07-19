<!-- markdownlint-disable MD033 -->
# Arduino-report-size-deltas

[![rust-ci-badge]][rust-ci-runs] [![codecov-badge]][codecov-link]

[rust-ci-badge]: https://github.com/2bndy5/arduino-report-size-deltas/actions/workflows/rust.yml/badge.svg
[rust-ci-runs]: https://github.com/2bndy5/arduino-report-size-deltas/actions/workflows/rust.yml
[codecov-badge]: https://codecov.io/github/2bndy5/arduino-report-size-deltas/graph/badge.svg?token=W9SAQIH91A
[codecov-link]: https://codecov.io/github/2bndy5/arduino-report-size-deltas

A CLI tool designed for CI workflows that posts comments on a Pull Request about the memory size of Arduino sketches.

This is a port of [`arduino/report-size-deltas` GitHub action][original GitHub Action] from python to rust.

[original GitHub Action]: https://github.com/arduino/report-size-deltas

## Example

The following example workflow will compile sketches, save reports as artifacts, and submit a summarizing comment.

```yml
name: Get Compile Size
on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  compile-sketches:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        fqbn: [arduino::avr:uno, arduino::samd::mkrzero]
    steps:
      - uses: actions/checkout@v4
      - uses: arduino/compile-sketches@v1
        with:
          fqbn: ${{ matrix.fqbn }}
          enable-deltas-report: true
      - uses: actions/upload-artifact@v4
        with:
          path: sketches-reports
          name: sketches-reports_${{ matrix.fqbn }}

  report-delta-size:
    needs: [compile-sketches]
    runs-on: ubuntu-latest
    permissions:
      # permission needed to create comments on a Pull Request
      pull-requests: write
    steps:
      - uses: actions/download-artifact@v4
        with:
          pattern: sketches-reports_*
      - name: Report size deltas
        uses: 2bndy5/arduino-report-size-deltas
```

### Triggered by a `pull_request`

When this action is triggered by a `pull_request` event,
it will post a thread comment summarizing the change in compiled sizes.

In this scenario, the given GitHub token (defaults to `github.token`) needs
write permission for `pull-requests` (see example above) to post a comment.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="examples/pr-comment-dark.png">
  <source media="(prefers-color-scheme: light)" srcset="examples/pr-comment-light.png">
  <img alt="(PR comment preview image not rendered)" src="examples/pr-comment-light.png">
</picture>

Any existing report comment is updated.
If for some reason, there are multiple report comments found, then
all report comments except the latest are deleted.
And only the latest found report comment is updated.

Only comments created by this action are manipulated.

### Triggered by anything except `pull_request`

When this action is **not** triggered by a `pull_request` event,
it will append the summary comment to the workflow's run summary page.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="examples/step-summary-dark.png">
  <source media="(prefers-color-scheme: light)" srcset="examples/step-summary-light.png">
  <img alt="(Run summary preview image not rendered)" src="examples/step-summary-light.png">
</picture>

In this scenario, no special permissions are needed.

## Why?

The [original GitHub Action] has some disadvantages:

- The [original GitHub Action] is lacking maintainer attention (beyond sporadic dependency updates).
- The [original GitHub Action] does not seem to be accepting new features. Contributions submitted by third-parties (community) get no maintainer feedback (other than maintainer-managed labels for issues or Pull Requests).
- The [original GitHub Action] requires a docker container to run the python code in isolation. This also means the [original GitHub Action] is only compatible with Linux-based CI runners.
- The [original GitHub Action] repository is bloated with metadata specifically used for development (not production).

## Goals

- [x] Mostly drop-in compatible with the [original GitHub Action].
  See [incomplete feature parity](#incomplete-feature-parity) below.
- [x] Simple enough to accept contributions from the community.
- [x] Does not require a docker container.
  Instead this CLI tool is deployed as a standalone binary that can be portably installed.
- [x] Data tables in the posted comment are more legible.
- [x] Support push events in a non-intrusive way:
  Instead of posting a comment to the commit's diff page, the report/comment is appended to
  the CI workflow run's summary page.

### Incomplete feature parity

Some features present in the [original GitHub Action] are not implemented in this port.
These features can be added upon request, but the utility of the feature should be "Generally Applicable";
meaning the feature does not just satisfy an individual use case.

### No [CSV] output

The [CSV] output used in the [original GitHub Action] is not included in this action's posted comment.
This is because [CSV] is a machine-readable syntax that is better served via programmatic means.
If you need [CSV] output, please submit a feature request so we can discuss the best approach toward integration.

[CSV]: https://en.wikipedia.org/wiki/Comma-separated_values

### No support for scheduled CI trigger

[schedule]: https://docs.github.com/en/actions/reference/events-that-trigger-workflows#schedule

Often called "cron jobs", the [original GitHub Action] supported GitHub's [schedule] workflow trigger.
In this case, the [original GitHub Action] would

1. Traverse open Pull Requests for the repository.
2. Scan for artifacts produced by `arduino/compile-sketches` action.
   This involves traversing each commit on each Pull Request to find the latest applicable artifacts.
3. Post a report/comment in the open Pull Request if one doesn't already exist.
   This implies traversing the open Pull Request's existing comments to find one that was
   produced by the [original GitHub Action].

This feature adds a lot of complexity to satisfy an unknown use case.
There are no feature requests in the [original GitHub Action] history (or its original experimental source)
that describe a problem this feature would solve.

Again, a request for this feature is welcome.
But, the rationale for such complexity should be stated clearly in a way that
is beneficial to the Arduino Community at large, not just beneficial to the
[Arduino GitHub Organization](https://github.com/arduino).

#### Problematic

Steps 2 and 3 could easily lock out the given token's read/write access by surpassing the
GitHub REST API's rate limits.

Additionally, force pushed commits on a feature branch can cause some workflow runs to
correspond with orphaned commits. This further increases the possibility of violating
the GitHub REST API's rate limits.

The discovered artifacts are not guaranteed to be produced by the `arduino/compile-sketches` action.
In the [original GitHub Action], the name of the artifact (not the JSON file name) is matched against the
`sketches-reports-source` input value.
This approach introduces a surface area for undefined behavior because the
artifacts' JSON data/content is not verified (for which the solution would further increase the
possibility of violating the GitHub REST API's rate limits).
