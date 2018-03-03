zvault-config(1) -- Display or change the configuration
=======================================================

## SYNOPSIS

`zvault config <REPO>`


## DESCRIPTION

This subcommand displays or changes the configuration of the repository `REPO`.
The configuration can be changes using the options described below. If no
options are set, the current configuration is displayed. Otherwise, the
configuration is changed as specified and then displayed.

Beware that the *chunker algorithm*, *chunk size* and *hash method* should not
be changed on existing repositories already containing many backups. If those
values are changed, new backups will not be able to use existing data for
deduplication. This can waste lots of storage space and most likely outweighs
the expected benefits.

The values for *bundle size*, *compression* and *encryption* only affect new
data and can be changed at any time without any drawback.


## OPTIONS

* `--bundle-size <SIZE>`:

  Set the target bundle size in MiB (default: 25).
  Please see _zvault(1)_ for more information on *bundle size*.


* `--chunker <CHUNKER>`:

  Set the chunker algorithm and target chunk size (default: fastcdc/16).
  Please see _zvault(1)_ for more information on *chunkers* and possible
  values.


* `-c`, `--compression <COMPRESSION>`:

  Set the compression method and level (default: brotli/3).
  Please see _zvault(1)_ for more information on *compression* and possible
  values.


* `-e`, `--encryption <PUBLIC_KEY>`:

  Use the given public key for encryption. The key must be a valid public key
  encoded as hexadecimal. Please use _zvault-genkey(1)_ to generate keys and
  _zvault-addkey(1)_ to add keys to the repository.

  If `none` is given as public key, encryption is deactivated.

  **Warning:** ZVault does not verify that the matching secret key which is
  needed for decryption is known.

  Please see _zvault(1)_ for more information on *encryption*.


* `--hash <HASH>`:

  Set the hash method (default: blake2).
  Please see _zvault(1)_ for more information on *hash methods* and possible
  values.


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
