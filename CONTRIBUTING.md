# Contributing

## Feature Matrix

`soft_ratatui` is feature-gated enough that the default build is not a sufficient release check.

- Run [`scripts/run_checks.sh`](/Users/kisaczka/Desktop/programming/soft_ratatui/scripts/run_checks.sh) from the repository root.
- The script runs formatting, the crate feature matrix, doctests, docs generation, and the workspace-wide compile check.

## Docs

- `./scripts/run_checks.sh` already includes doctests and `cargo doc -p soft_ratatui --all-features --no-deps`.
- docs.rs is configured to build with `--all-features` and show feature gates with `doc(cfg(...))`.

## Workspace

The example applications under `more_examples_click_me/` are workspace members and are validated by `./scripts/run_checks.sh`.

## Release Checklist

1. Run `./scripts/run_checks.sh`.
2. Review warnings from examples and feature-specific builds.
3. Update README examples if public constructors or feature names changed.
