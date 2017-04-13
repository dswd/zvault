zvault-algotest(1) -- Test a specific algorithm combination
===========================================================

## SYNOPSIS

`zvault algotest [OPTIONS] <FILE>`


## DESCRIPTION

This subcommand tests a specific combination of algorithms on a given input
`FILE`.

The subcommand exists to help users compare and select algorithms and
configuration options when creating a repository with _zvault-init(1)_ or
changing its configuration via _zvault-config(1)_.

The given algorithms will be used to simulate a backup run and determine the
efficiency and performance of each used algorithm as well as their combination.

The input file `FILE` is used as sample data during the test and should be
selected to be representative for the envisioned use case. Good examples of such
files can be tar files of system images or parts of a home folder.
Please note, that the input file is read into memory completely in order to
factor out the hard drive speed of the analysis.

The options are exactly the same as for _zvault-init(1)_.


## OPTIONS

* `--bundle-size <SIZE>`:

  Set the target bundle size in MiB (default: 25).
  Please see zvault(1) for more information on *bundle size*.


* `--chunker <CHUNKER>`:

  Set the chunker algorithm and target chunk size (default: fastcdc/16).
  Please see _zvault(1)_ for more information on *chunkers* and possible
  values.


* `-c`, `--compression <COMPRESSION>`:

  Set the compression method and level (default: brotli/3).
  Please see _zvault(1)_ for more information on *compression* and possible
  values.


* `-e`, `--encrypt`:

  Generate a keypair and enable encryption.
  Please see _zvault(1)_ for more information on *encryption*.


* `--hash <HASH>`:

  Set the hash method (default: blake2).
  Please see _zvault(1)_ for more information on *hash methods* and possible
  values.


* `-h`, `--help`:

  Prints help information


## COPYRIGHT

Copyright (C) 2017  Dennis Schwerdel
This software is licensed under GPL-3 or newer (see LICENSE.md)
