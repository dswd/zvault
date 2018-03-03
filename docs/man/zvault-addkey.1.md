zvault-addkey(1) -- Add a key pair to the repository
====================================================

## SYNOPSIS

`zvault addkey [OPTIONS] [FILE] <REPO>`


## DESCRIPTION

This subcommand adds a new key pair to the repository `REPO`.

If `FILE` is given, the key pair is read from the file and added to the
repository.

If `--generate` is set, a new key pair is generated, printed to console and
added to the repository. If `--password` is also set, the key pair will be
derived from the given password instead of creating a random one.

If `--default` is set, encryption will be enabled (if not already) and the new
key will be set as default encryption key.


## OPTIONS

* `-g`, `--generate`:

  Generate a new key pair


* `-d`, `--default`:

  Set the key pair as default


* `-p`, `--password <PASSWORD>`:

  Derive the key pair from the given password instead of randomly creating it.
  This setting requires that `--generate` is set too.


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
