zvault-init(1) -- Initialize a new repository
=============================================

## SYNOPSIS

`zvault init [OPTIONS] --remote <REMOTE> [REPO]`


## DESCRIPTION

This subcommand initializes a new repository at the location `REPO`. If `REPO`
is omitted, the default repository location will be used. It is important that
the path given as `REPO` does not yet exist, so that it can be created.

The remote storage path `REMOTE` must be an existing empty folder. ZVault
supports mounted remote filesystems, so it is a good idea to use such a folder
to keep the backups on a remote location.

This subcommand should **NOT** be used to import existing remote backup
locations. Please use _zvault-import(1)_ for this purpose.

The rest of the options sets configuration options for the new repository. The
configuration can be changed by _zvault-config(1)_ later.


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


  * `-e`, `--encryption`:

    Generate a keypair and enable encryption.
    Please see _zvault(1)_ for more information on *encryption*.


  * `--hash <HASH>`:

    Set the hash method (default: blake2).
    Please see _zvault(1)_ for more information on *hash methods* and possible
    values.


  * `-h`, `--help`:

    Prints help information


  * `-r`, `--remote <REMOTE>`:

    Set the path to the mounted remote storage. There should be an empty folder
    at this location.


## COPYRIGHT

Copyright (C) 2017  Dennis Schwerdel
This software is licensed under GPL-3 or newer (see LICENSE.md)
