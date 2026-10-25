# Contributing

Thanks for your interest in tracr. Bug reports and ideas are welcome as
[issues](https://github.com/JoKeks2023/tracr/issues); please read
[SECURITY.md](SECURITY.md) for anything security-relevant.

## License of contributions

tracr is released under `AGPL-3.0-or-later` (see [LICENSE](LICENSE)).

By submitting a contribution (a pull request, patch or code in an issue) you
confirm that you wrote it or have the right to submit it, and you license it
to the project owner under `AGPL-3.0-or-later` **and** grant the owner the
right to also distribute it under other terms, including commercial licenses.
This keeps the option open to offer tracr under additional terms later; the
public release stays AGPL.

If you do not agree, please open an issue to discuss before sending code.

## Before sending a pull request

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
pnpm --dir apps/desktop typecheck
```

tracr never stores prompt or tool content; changes must not add that.
