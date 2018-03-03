zvault-copy(1) -- Create a copy of a backup
===========================================

## SYNOPSIS

`zvault copy [OPTIONS] <SRC> <DST>`


## DESCRIPTION

This subcommand copies the backup `SRC` to a new name `DST`.

The backups given by `SRC` and `DST` must be in the format
`[repository]::backup_name[::subtree]` as described in _zvault(1)_.
If `repository` is omitted, the default repository location is used instead.


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
