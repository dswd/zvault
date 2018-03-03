zvault-import(1) -- Reconstruct a repository from the remote storage
====================================================================

## SYNOPSIS

`zvault import <REMOTE> <REPO>`


## DESCRIPTION

This subcommand imports a repository from remote storage. First, an empty
repository will be created and then the remote bundles will be imported and
added to the local index.

The repository will be created at the location `REPO`. It is important that the
path given as `REPO` does not yet exist, so that it can be created.

The remote storage path `REMOTE` must be an existing remote storage folder
initialized by _zvault-init(1)_.

Note that this command is not intended to import single backups exported as tar
files via _zvault-restore(1)_ with the `--tar` flag. Those archives can be
imported via _zvault-backup(1)_ also with the `--tar` flag.


## OPTIONS

* `-k`, `--key <FILE>...`:

  Add the key pair in the given file to the repository before importing the
  remote bundles. This option can be used to add keys that are needed to read
  the bundles. If multiple keys are needed, this options can be given multiple
  times.


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
