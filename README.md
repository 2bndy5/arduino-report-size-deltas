# Arduino-report-size-deltas

A CLI tool designed for CI workflows that posts comments on a Pull Request about the memory size of Arduino sketches.

This is a port of [`arduino/report-size-deltas` GitHub action][original GitHub Action] from python to rust.

[original GitHub Action]: https://github.com/arduino/report-size-deltas

## Why?

The [original GitHub Action] has some disadvantages:

- The [original GitHub Action] is lacking maintainer attention (beyond sporadic dependency updates).
- The [original GitHub Action] does not seem to be accepting new features. Contributions submitted by third-parties (community) get no maintainer feedback (other than maintainer-managed labels for issues or Pull Requests).
- The [original GitHub Action] requires a docker container to run the python code in isolation. This also means the [original GitHub Action] is only compatible with Linux-based CI runners.
- The [original GitHub Action] repository is bloated with metadata specifically devoted to various tooling required for typical development workflow.

## Goals

- [x] Drop-in compatible with the [original GitHub Action]
- [x] Simple enough to accept contributions from the community.
- [x] Does not require a docker container. Instead this CLI tool is deployed as a standalone binary that can be portably installed.
- [x] Data tables in the posted comment are more legible. Note, the [CSV] output used in the [original GitHub Action] is not included in the comment because [CSV] is a machine-readable syntax that is better served via CI artifact(s).

If you need [CSV] output, please submit a feature request so we can discuss the best approach toward integration.

[CSV]: https://en.wikipedia.org/wiki/Comma-separated_values
