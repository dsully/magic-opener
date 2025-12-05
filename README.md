# magic-opener

A tool for opening the right thing in the right place.

## Features

* Opens the repository's web page for the current branch (PR page if on a PR branch)
* Opens commit URLs (e.g., `open 297b17b35`)
* Detects PR numbers in commit messages and opens the PR directly
* Opens PRs by number (e.g., `open 123`)
* Falls back to standard macOS `open` behavior for non-Git paths
* `--print` flag to output the URL without opening

All arguments are optional.

## Install

### Homebrew

```shell
brew install dsully/tap/magic-opener
```

### Source

```shell
cargo install --git https://github.com/dsully/magic-opener
```

### Nix Flakes

```shell
nix profile install github:dsully/magic-opener
```

## Usage

```shell
open [--print] [path|commit|pr-number]
```
