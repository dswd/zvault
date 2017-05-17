zvault-versions(1) -- Find different versions of a file in all backups
======================================================================

## SYNOPSIS

`zvault versions [OPTIONS] <REPO> <PATH>`


## DESCRIPTION

This subcommand finds and lists all versions of the file given by `PATH` in any
backup in the repository `REPO`.

The path given by `PATH` must be relative with regard to the repository root.

All different versions of the file in all backups will be listed by this
subcommand. That means that only unique versions will be listed with the
earliest backup that version appeared in.


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
