# Changelog

This project follows [semantic versioning](http://semver.org).


### UNRELEASED
* [added] Added `copy` subcommand
* [added] Added support for xattrs in fuse mount
* [modified] Also documenting common flags in subcommands
* [modified] Using repository aliases (**conversion needed**)
* [modified] Remote path must be absolute
* [fixed] Fixed tarfile import


### v0.3.2 (2017-05-11)
* [modified] Changed order of arguments in `addkey` to match src-dst scheme
* [modified] Skip root folder on restore
* [fixed] Fixed `addkey` subcommand
* [fixed] Fixed reading tar files from stdin
* [fixed] Fixed exporting files with long names as tar files


### v0.3.1 (2017-05-09)
* [added] Derive key pairs from passwords
* [modified] Added root repository to exclude list
* [modified] Initializing data in index before use
* [modified] Updated dependencies


### v0.3.0 (2017-04-27)
* [added] Ability to read/write tar file from/to stdin/stdout
* [added] Added date to bundles
* [added] Option to combine small bundles
* [added] Fixed chunker
* [modified] Logging to stderr
* [modified] Enforce deterministic bundle ordering
* [modified] More info in analyze subcommand
* [modified] Estimating final bundle size in order to reach it
* [fixed] Only print "repairing bundles" if actually repairing bundles
* [fixed] Only put mode bits of st_mode into metadata
* [fixed] Only repairing backups with --repair
* [fixed] Fixed vacuum
* [fixed] First removing bundles, then adding new ones
* [fixed] No longer clobbering broken files


### v0.2.0 (2017-04-14)
* [added] Added CHANGELOG
* [added] Locking local repository to avoid index corruption
* [added] Storing user/group names in backups
* [added] Ability to repair bundles, backups, index, bundle map and bundle cache
* [added] Manpages for all subcommands
* [added] Folders of backups can be listed, removed and mounted
* [added] Supporting extended attributes in tar files
* [modified] No longer trying to upload by rename
* [modified] No longer failing restore if setting file attributes fails
* [modified] Backup files must end with `.backup` (**conversion needed**)
* [modified] Bundle files must end with `.bundle`
* [modified] Ignoring corrupt bundles instead of failing
* [fixed] Creating empty bundle cache on init to avoid warnings
* [fixed] Calling sodiumoxide::init for faster algorithms and thread safety (not needed)
* [fixed] Fixed a deadlock in the bundle upload code
* [fixed] Also setting repository dirty on crash
* [fixed] Ignoring missing backups folder
* [fixed] Fixed problems with uploads from relative repository paths
* [fixed] Fixed finished messages
* [fixed] Fixed inode retrieval for single-file backups
* [fixed] Fixed tar import


### v0.1.0 (2017-04-11)
First working alpha release

This release features the main functionality:
* Initializing repository
  - Generating a key on the fly
  - Import existing repository
* Creating backups
  - Partial backups
  - Deduplication
  - Compression
  - Encryption
  - From tar files
  - Support for file permissions, file date and extended attributes
* Restoring backups
  - Full or subtrees
  - To tar files
* Mounting backups or the whole repository
* Removing backups
  - Full or only specific subtrees
  - By date (`prune` subcommand)
* Check integrity
  - Repository
  - Bundles
  - Index
  - Backups
  - Inode trees
* Vacuum
  - By ratio
* Listing & Info methods
  - Repository info
  - Backup info/list
  - Directory list, Inode info
  - Bundle list and info
* Utility commands
  - `analyze`: analyze chunk usage
  - Key management commands (`addkey`, `genkey`)
  - `algotest`: algorithm testing
  - `versions`: find versions of a file
  - `diff`: Find differences between backups
  - `config`: Getting and setting config options
* Command line client
  - Powerful argument parsing
  - Nice colorful error messages
  - Progress bars
  - Man pages for main commands
* Special functionality
  - Shared repositories
