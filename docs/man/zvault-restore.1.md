zvault-restore(1) -- Restore a backup or subtree
================================================

## SYNOPSIS

`zvault restore [OPTIONS] <BACKUP> <DST>`


## DESCRIPTION

This subcommand restores a backup or a backup subtree `BACKUP` into the folder
`DST`.

The backup or backup subtree given by `BACKUP` must be in the format
`[repository]::backup_name[::subtree]` as described in _zvault(1)_.
If `repository` is omitted, the default repository location is used instead.

If `--tar` is set, the data is written to a tar file named `DST`. In this case
`DST` must not exist. If `DST` is `-`, the data will be written to stdout.

If `--tar` is not set, the data will be written into the existing folder `DST`.


## OPTIONS

  * `--tar`:

    Write the backup to a tar archive named `DST` instead of creating files and
    folders at this location.

    This option can be used to export a backup that can be imported again using
    zvault-backup(1) with the `--tar` flag.


  * `-h`, `--help`:

    Prints help information


## COPYRIGHT

Copyright (C) 2017  Dennis Schwerdel
This software is licensed under GPL-3 or newer (see LICENSE.md)
