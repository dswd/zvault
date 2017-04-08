# TODO

## Functionality
* Detach bundle upload
* XAttrs in fuse
* XAttrs in tar
* `check --repair`

## Stability / Reliability
* Lock the local repository to avoid index corruption
* Recover from missing index, bundle cache and bundle map by rebuilding those

## Usability
* Verbosity control
* Display backup name and path on backup integrity error
* Better control over what is checked in `check` subcommand
* Nice error when remote storage is not mounted
* Man pages for all minor subcommands

## Code quality
* Test cases
* Benchmarks
* Full fuse method coverage
* Clippy
* Do not estimate meta size

## Other
* Homepage
