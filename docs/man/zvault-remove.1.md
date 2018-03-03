zvault-remove(1) -- Remove a backup or a subtree
================================================

## SYNOPSIS

`zvault remove [OPTIONS] <BACKUP>`


## DESCRIPTION

This subcommand removes a backup or a backup subtree `BACKUP`.

The backup or backup subtree given by `BACKUP` must be in the format
`[repository]::backup_name[::subtree]` as described in _zvault(1)_.
If `repository` is omitted, the default repository location is used instead.

If a backup is referenced, this backup will be deleted. If a subtree is given,
the backup is instead rewritten to not include that subtree anymore.

If a folder of backups is referenced by `BACKUP` the flag `--force` must be set
in order to remove all backups in that folder (also recursively).

Note: When removing backup subtrees, the meta information of that backup is left
unchanged and still contains the data (e.g. duration and size) of the original
backup run.

This command renders certain chunks unused, but reclaiming their space is a
complicated task as chunks are combined into bundles together with other chunks
which are potentially still used. Please use _zvault-vacuum(1)_ to reclaim
unused space.

**Important note: Although this command does not actually remove any data, the
data of the deleted backups becomes inaccessible and can not be restored.**


## OPTIONS

* `-f`, `--force`:

  Remove multiple backups in a backup folder


* `-q`, `--quiet`:

  Print less information


* `-v`, `--verbose`:

  Print more information


* `-h`, `--help`:

  Prints help information


* `-V`, `--version`:     

  Prints version information


## COPYRIGHT

Copyright (C) 2017-2018  Dennis Schwerdel
This software is licensed under GPL-3 or newer (see LICENSE.md)
