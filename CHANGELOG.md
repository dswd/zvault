# Changelog

This project follows [semantic versioning](http://semver.org).


### UNRELEASED
- [added] Added CHANGELOG
- [added] Locking local repository to avoid index corruption
- [fixed] Creating empty bundle cache on init to avoid warninigs
- [fixed] Calling sodiumoxide::init for faster algorithms and thread safety (not needed)


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