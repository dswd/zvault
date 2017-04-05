zvault-check(1) -- Check the repository, a backup or a backup subtree
=====================================================================

## SYNOPSIS

`zvault check [OPTIONS] [PATH]`


## DESCRIPTION

This subcommand checks the repository, a backup or a backup subtree given by
`PATH`.

The repository, backup, of subtree given by `PATH` must be in the format
`[repository][::backup_name[::subtree]]` as described in _zvault(1)_.
If `PATH` is omitted, the default repository location is used instead.

The command will perform the following checks in order:
- Bundle integrity
- Full bundle contents (optional)
- Index integrity
- Backup integrity
- Filesystem integrity

If a backup is specified in `PATH`, only this backup will be check in the backup
integrity check and only the filesystem integrity of this backup will be checked
in the filesystem integrity check.

If a subtree is specified in `PATH`, no backups will be checked and only the
given subtree will be checked in the filesystem integrity check.

Unless `--full` is set, the bundles will only be checked without actually
fetching them fully. This means that their contents can only be read from their
header and this information is not verified. If `--full` is set, the full
bundles are fetched and their contents are compared to what their header claims.
This check takes a long time since all bundles need to fetched, decrypted and
decompressed fully to read their contents.


## OPTIONS

  * `--full`:

    Also check the contents of the bundles by fetching and decompressing them.
    Note: This flag causes the check to be much slower.


  * `-h`, `--help`:

    Prints help information


## COPYRIGHT

Copyright (C) 2017  Dennis Schwerdel
This software is licensed under GPL-3 or newer (see LICENSE.md)
