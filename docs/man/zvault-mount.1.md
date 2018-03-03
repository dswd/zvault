zvault-mount(1) -- Mount the repository, a backup or a subtree
==============================================================

## SYNOPSIS

`zvault mount <PATH> <MOUNTPOINT>`


## DESCRIPTION

This subcommand mounts a repository, backup or backup subtree specified by
`PATH` on the location given by `MOUNTPOINT` making it accessible as a
filesystem.

The repository, backup or backup subtree given by `PATH` must be in the format
`[repository][::backup_name[::subtree]]` as described in _zvault(1)_.

If `PATH` specifies a backup or backup subtree, the root of that backup or the
respective subtree is mounted onto the given location.
If `PATH` specifies a whole repository, all backups of that repository will be
accessible in separate folders below the given mount point.

The provided file system is mounted read-only, i.e. it can only be used to
inspect and restore backups but not to create new backups or modify exiting
ones.

Please note that since the filesystem is mounted via fuse, restoring huge data
this way is slower than using _zvault-restore(1)_.


## OPTIONS

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
