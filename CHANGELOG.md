# Changelog

This project follows [semantic versioning](http://semver.org).


### UNRELEASED
- [added] Added CHANGELOG
- [added] Locking local repository to avoid index corruption
- [added] Storing user/group names in backups
- [added] Ability to repair bundles, backups, index, bundle map and bundle cache
- [added] Manpages for all subcommands
- [modified] No longer trying to upload by rename
- [modified] No longer failing restore if setting file attributes fails
- [modified] Backup files must end with `.backup` (**conversion needed**)
- [modified] Bundle files must end with `.bundle`
- [modified] Ingnoring corrupt bundles instead of failing
- [fixed] Creating empty bundle cache on init to avoid warnings
- [fixed] Calling sodiumoxide::init for faster algorithms and thread safety (not needed)
- [fixed] Fixed a deadlock in the bundle upload code
- [fixed] Also setting repository dirty on crash
- [fixed] Ignoring missing backups folder
- [fixed] Fixed problems with uploads from relative repository paths
- [fixed] Fixed finished messages


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
