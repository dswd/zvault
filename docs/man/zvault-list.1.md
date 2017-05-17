zvault-list(1) -- List backups or backup contents
=================================================

## SYNOPSIS

`zvault list <PATH>`


## DESCRIPTION

This subcommand lists all backups or backup contents of the repository or backup
specified by `PATH`.

The repository, backup or backup subtree given by `PATH` must be in the format
`[repository][::backup_name[::subtree]]` as described in _zvault(1)_.

If `PATH` specifies a repository, all backups of this repository are listed.

If `PATH` specifies a backup or a backup subtree, all contents of this folder
are displayed. In the case of a backup, the contents of its root folder are
displayed.

_zvault-info(1)_ can be used to display more information on single entities.

Note that _zvault-mount(1)_ can be used to make backups accessible as a
filesystem which is faster than _zvault-list(1)_ for multiple listings.


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

Copyright (C) 2017  Dennis Schwerdel
This software is licensed under GPL-3 or newer (see LICENSE.md)
