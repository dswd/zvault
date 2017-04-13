zvault-bundleinfo(1) -- Display information on a bundle
=======================================================

## SYNOPSIS

`zvault bundleinfo [OPTIONS] <REPO> <BUNDLE>`


## DESCRIPTION

This subcommand displays information on bundle `BUNDLE` in the repository
`REPO`.

The argument `BUNDLE` must give the id of an existing bundle as listed by
_zvault-bundlelist(1)_. Please note that bundles are stored with random file
names on the remote storage that do not relate to the bundle id.



## OPTIONS

  * `-h`, `--help`:

    Prints help information


## COPYRIGHT

Copyright (C) 2017  Dennis Schwerdel
This software is licensed under GPL-3 or newer (see LICENSE.md)
