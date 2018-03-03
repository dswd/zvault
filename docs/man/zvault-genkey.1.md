zvault-genkey(1) -- Generate a new key pair
===========================================

## SYNOPSIS

`zvault genkey [OPTIONS] [FILE]`


## DESCRIPTION

This subcommand generates a new key pair, prints it to console and optionally
writes it to the given file `FILE`.


## OPTIONS

* `-p`, `--password <PASSWORD>`:

  Derive the key pair from the given password instead of randomly creating it.


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
