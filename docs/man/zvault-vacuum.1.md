zvault-vacuum(1) -- Reclaim space by rewriting bundles
======================================================

## SYNOPSIS

`zvault vacuum [OPTIONS] <REPO>`


## DESCRIPTION

This subcommand reclaims space by rewriting bundles in the repository `REPO`.

This command rewrites bundles to remove unused chunks of backups that have been
removed by _zvault-remove(1)_ or _zvault-prune(1)_.
To accomplish this, it will scan all backups and track all used chunks to
identify chunks that are not used by any backup. Those chunks are then grouped
by bundle and bundles with many unused chunks will be rewritten with those
chunks left out.

The option `--ratio` configures the minimal ratio of used chunks in a bundle
required to remove it. Since all chunks that are still used must be read from
the bundle and written to a new one and only the storage space of the unused
chunks can be reclaimed, rewriting a bundle is more economical the lower the
ratio. At a ratio of 0% will only rewrite bundles with no used chunks at all
(in this case the bundle is just removed). At a ratio of 100%, all bundles will
be rewritten regardless of unused chunks.

Please note that the bundles will be rewritten with the current settings for
encryption and compression, disregarding the original settings during bundle
creation.

Unless `--force` is set, this command will only simulate the process but not
actually rewrite any bundle.

As this is a critical operation, zVault takes many precaution measures to avoid
any damaging the integrity to the repository or other backups. The whole process
is performed with an exclusive lock on the repository which prevents any backup
runs. Also the chunk index is double checked before removing bundles to make
sure that they are unused. Nevertheless, this is a critical operation which
should be avoided when the storage space permits it.



## OPTIONS

* `--combine`:

  Also combine small bundles into larger ones.


* `-r`, `--ratio <NUM>`:

  Do not rewrite bundles with more than `NUM`% of used chunks.
  The ratio must be given in whole percentage, e.g. 50 mean 50%.


* `-f`, `--force`:

  Actually run the vacuum instead of simulating it.


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
