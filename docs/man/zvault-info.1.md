zvault-info(1) -- Display information on a repository, a backup or a subtree
============================================================================

## SYNOPSIS

`zvault info <PATH>`


## DESCRIPTION

This subcommand displays information on the repository, backup or backup subtree
specified by `PATH`.

The repository, backup or backup subtree given by `PATH` must be in the format
`[repository][::backup_name[::subtree]]` as described in _zvault(1)_.


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
