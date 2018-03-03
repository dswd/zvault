zvault-analyze(1) -- Analyze the used and reclaimable space of bundles
======================================================================

## SYNOPSIS

`zvault analyze [OPTIONS] <REPO>`


## DESCRIPTION

This subcommand analyzes the used and reclaimable storage space of bundles in
the repository `REPO`.

The analysis will scan through all backups and identify used chunks, order them
by bundle and finally determine and print the space that could be reclaimed by
running _zvault-vacuum(1)_ with different ratios.


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
