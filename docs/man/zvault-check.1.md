zvault-check(1) -- Check the repository, a backup or a backup subtree
=====================================================================

## SYNOPSIS

`zvault check [OPTIONS] <PATH>`


## DESCRIPTION

This subcommand checks the repository, a backup or a backup subtree given by
`PATH`.

The repository, backup, of subtree given by `PATH` must be in the format
`[repository][::backup_name[::subtree]]` as described in _zvault(1)_.

The command will perform the following checks in order:
- Bundle integrity (optional)
- Full bundle contents (optional)
- Index integrity (optional)
- Backup integrity
- Filesystem integrity

If a backup is specified in `PATH`, only this backup will be check in the backup
integrity check and only the filesystem integrity of this backup will be checked
in the filesystem integrity check.

If a subtree is specified in `PATH`, no backups will be checked and only the
given subtree will be checked in the filesystem integrity check.

If `--bundles` is set, the integrity of the bundles will be checked before
checking any backups.
If `--bundle-data` is also set, the full bundles are fetched and their contents
are compared to what their header claims. This check takes a long time since all
bundles need to fetched, decrypted and decompressed fully to read their
contents. If this flag is not set, the bundles will only be checked without
actually fetching them fully. This means that their contents can only be read
from their header and this information is not verified.

If `--index` is set, the integrity of the index and its contents will be checked
before checking any backups.

If `--repair` is set, zVault will try to repair and rebuild things instead of
failing when problems are detected. The repair process will rebuild all local
repository components (index, bundle cache, bundle map) when problems are
identified with them.
If any bundle is broken, a new bundle is created with as much of the readable
data of that bundle as possible. The old bundle is not removed but gets the file
extension `.bundle.broken`.
If any backup is broken, a new backup is created with a much of the readable
files and directories of that backup as possible. The old backup is not removed
but gets the file extension `.backup.broken`.
Please note the destructive nature of the repair process as it removes all
references to data that has been corrupted in order to make the rest of the data
accessible again. To make this process reversible, the old data is kept and just
renamed.
Manual computer forensics might be able to restore some of the corrupt or
inaccessible data that repair can not restore automatically.
Please note that any run of _zvault-vacuum(1)_ will remove any intact data that
has become inaccessible.


## OPTIONS

* `-b`, `--bundles`:

  Check the integrity of the bundles too.


* `--bundle-data`:

  Also check the contents of the bundles by fetching and decompressing them.
  Note: This flag causes the check to be much slower.


* `-i`, `--index`:

  Also check the integrity of the index and its contents.


* `-r`, `--repair`:

  Try to repair broken bundles, backups and rebuild local data when necessary.


* `-q`, `--quiet`:

  Print less information


* `-v`, `--verbose`:

  Print more information


* `-h`, `--help`:

  Prints help information


* `-V`, `--version`:     

  Prints version information


## COPYRIGHT

Copyright (C) 2017  Dennis Schwerdel
This software is licensed under GPL-3 or newer (see LICENSE.md)
