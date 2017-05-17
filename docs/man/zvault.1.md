zvault(1) -- Deduplicating backup solution
==========================================

## SYNOPSIS

`zvault <SUBCOMMAND>`



## DESCRIPTION

ZVault is a deduplicating backup solution. It creates backups from data read
from the filesystem or a tar file, deduplicates it, optionally compresses and
encrypts the data and stores the data in bundles at a potentially remote storage
location.



## OPTIONS

  * `-h`, `--help`:

    Prints help information


  * `-V`, `--version`:     

    Prints version information



## SUBCOMMANDS


### Main Commands

  * `init`          Initialize a new repository, _zvault-init(1)_
  * `import`        Reconstruct a repository from the remote storage, _zvault-import(1)_
  * `backup`        Create a new backup, _zvault-backup(1)_
  * `restore`       Restore a backup or subtree, _zvault-restore(1)_
  * `check`         Check the repository, a backup or a backup subtree, _zvault-check(1)_
  * `list`          List backups or backup contents, _zvault-list(1)_
  * `info`          Display information on a repository, a backup or a subtree, _zvault-info(1)_
  * `mount`         Mount the repository, a backup or a subtree, _zvault-mount(1)_
  * `remove`        Remove a backup or a subtree, _zvault-remove(1)_
  * `copy`          Create a copy of a backup, _zvault-copy(1)_
  * `prune`         Remove backups based on age, _zvault-prune(1)_
  * `vacuum`        Reclaim space by rewriting bundles, _zvault-vacuum(1)_


### Other Commands

  * `addkey`        Add a key pair to the repository, _zvault-addkey(1)_
  * `algotest`      Test a specific algorithm combination, _zvault-algotest(1)_
  * `analyze`       Analyze the used and reclaimable space of bundles, _zvault-analyze(1)_
  * `bundleinfo`    Display information on a bundle, _zvault-bundleinfo(1)_
  * `bundlelist`    List bundles in a repository, _zvault-bundlelist(1)_
  * `config`        Display or change the configuration, _zvault-config(1)_
  * `diff`          Display differences between two backup versions, _zvault-diff(1)_
  * `genkey`        Generate a new key pair, _zvault-genkey(1)_
  * `versions`      Find different versions of a file in all backups, _zvault-versions(1)_


## USAGE

### Path syntax

Most subcommands work with a repository that has to be specified as a parameter.
If this repository is specified as `::`, the default repository in `~/.zvault`
will be used instead.

Some subcommands need to reference a specific backup in the repository. This is
done via the syntax `repository::backup_name` where `repository` is the path to
the repository and `backup_name` is the name of the backup in that repository
as listed by `zvault list`. In this case, `repository` can be omitted,
shortening the syntax to `::backup_name`. In this case, the default repository
is used.

Some subcommands need to reference a specific subtree inside a backup. This is
done via the syntax `repository::backup_name::subtree` where
`repository::backup_name` specifies a backup as described before and `subtree`
is the path to the subtree of the backup. Again, `repository` can be omitted,
yielding the shortened syntax `::backup_name::subtree`.

Some subcommands can take either a repository, a backup or a backup subtree. In
this case it is important to note that if a path component is empty, it is
regarded as not set at all.

Examples:

- `~/.zvault` references the repository in `~/.zvault` and is identical with
  `::`.
- `::backup1` references the backup `backup1` in the default repository
- `::backup1::/` references the root folder of the backup `backup1` in the
  default repository


## CONFIGURATION OPTIONS
ZVault offers some configuration options that affect the backup speed, storage
space, security and RAM usage. Users should select them carefully for their
scenario. The performance of different combinations can be compared using
_zvault-algotest(1)_.


### Bundle size
The target bundle size affects how big bundles written to the remote storage
will become. The configured size is not a hard maximum, as headers and some
effects of compression can cause bundles to become slightly larger than this
size. Also since bundles will be closed at the end of a backup run, some bundles
can also be smaller than this size. However most bundles will end up with
approximately the specified size.

The configured value for the bundle size has some practical consequences.
Since the whole bundle is compressed as a whole (a so-called *solid archive*),
the compression ratio is impacted negatively if bundles are small. Also the
remote storage could become inefficient if too many small bundle files are
stored. On the other side, since the whole bundle has to be fetched and
decompressed to read a single chunk from that bundle, bigger bundles increase
the overhead of reading the data.

The recommended bundle size is 25 MiB, but values between 5 MiB and 100 MiB
should also be feasable.


### Chunker
The chunker is the component that splits the input data into so-called *chunks*.
The main goal of the chunker is to produce as many identical chunks as possible
when only small parts of the data changed since the last backup. The identical
chunks do not have to be stored again, thus the input data is deduplicated.
To achieve this goal, the chunker splits the input data based on the data
itself, so that identical parts can be detected even when their position
changed.

ZVault offers different chunker algorithms with different properties to choose
from:

- The **rabin** chunker is a very common algorithm with a good quality but a
  mediocre speed (about 350 MB/s).
- The **ae** chunker is a novel approach that can reach very high speeds
  (over 750 MB/s) at a cost of deduplication rate.
- The **fastcdc** algorithm reaches a similar deduplication rate as the rabin
  chunker but is faster (about 550 MB/s).

The recommended chunker is **fastcdc**.

Besides the chunker algorithm, an important setting is the target chunk size,
i.e. the planned average chunk size. Since the chunker splits the data on
data-dependent criteria, it will not achieve the configured size exactly.
The chunk size has a number of practical implications. Since deduplication works
by identifying identical chunks, smaller chunk sizes will be able to find more
identical chunks and thereby reduce the overall storage space.

On the other side, the index needs to store 24 bytes per chunk, so many small
chunks will take more space than few big chunks. Since the index of all chunks
in the repository needs to be loaded into memory during the backup, huge
repositories can get a problem with memory usage. Since the index could be only
40% filled and the chunker could yield smaller chunks than configured, 100 bytes
per chunk should be a safe value to calculate with.

The configured value for chunk size needs to be a power of 2. Here is a
selection of chunk sizes and their estimated RAM usage:

- Chunk size 4 KiB => ~40 GiB data stored in 1 GiB RAM
- Chunk size 8 KiB => ~80 GiB data stored in 1 GiB RAM
- Chunk size 16 KiB => ~160 GiB data stored in 1 GiB RAM
- Chunk size 32 KiB => ~325 GiB data stored in 1 GiB RAM
- Chunk size 64 KiB => ~650 GiB data stored in 1 GiB RAM
- Chunk size 128 KiB => ~1.3 TiB data stored in 1 GiB RAM
- Chunk size 256 KiB => ~2.5 TiB data stored in 1 GiB RAM
- Chunk size 512 KiB => ~5 TiB data stored in 1 GiB RAM
- Chunk size 1024 KiB => ~10 TiB data stored in 1 GiB RAM

The recommended chunk size for normal computers is 16 KiB. Servers with lots of
data might want to use 128 KiB or 1024 KiB instead.

The chunker algortihm and chunk size are configured together in the format
`algorithm/size` where algorithm is one of `rabin`, `ae` and `fastcdc` and size
is the size in KiB e.g. `16`. So the recommended configuration is `fastcdc/16`.

Please not that since the chunker algorithm and chunk size affect the chunks
created from the input data, any change to those values will make existing
chunks inaccessible for deduplication purposes. The old data is still readable
but new backups will have to store all data again.


### Compression
ZVault offers different compression algorithms that can be used to compress the
stored data after deduplication. The compression ratio that can be achieved
mostly depends on the input data (test data can be compressed well and media
data like music and videos are already compressed and can not be compressed
significantly).

Using a compression algorithm is a trade-off between backup speed and storage
space. Higher compression takes longer and saves more space while low
compression is faster but needs more space.

ZVault supports the following compression methods:

- **deflate** (also called *zlib* and *gzip*) is the most common algorithm today
  and guarantees that backups can be decompressed in future centuries. Its
  speed and compression ratio are acceptable but other algorithms are better.
  This is a rather conservative choice. This algorithm supports the levels 1
  (fastest) to 9 (best).
- **lz4** is a very fast compression algorithm that does not impact backup speed
  very much. It does not compress as good as other algorithms but is much faster
  than all other algorithms. This algorithm supports levels 1 (fastest) to 14
  (best) but levels above 7 are significantly slower and not recommended.
- **brotli** is a modern compression algorithm that is both faster and
  compresses better than deflate. It offers a big range of compression ratios
  and speeds via its levels. This algorithm supports levels 1 (fastest) to 10
  (best).
- **lzma** is about the algorithm with the best compression today. That comes
  at the cost of speed. LZMA is rather slow at all levels so it can slow down
  the backup speed significantly. This algorithm supports levels 1 (fastest) to
  9 (best).

The recommended combinations are:

- Focusing speed: lz4 with level between 1 and 7
- Balanced focus: brotli with levels between 1 and 10
- Focusing storage space: lzma with levels between 1 and 9

The compression algorithm and level are configured together via the syntax
`algorithm/level` where `algorithm` is either `deflate`, `lz4`, `brotli` or
`lzma` and `level` is a number.

The default compression setting is **brotli/3**.

Since the compression ratio and speed hugely depend on the input data,
_zvault-algotest(1)_ should be used to compare algorithms with actual input
data.



### Encryption
When enabled, zVault uses modern encryption provided by *libsodium* to encrypt
the bundles that are stored remotely. This makes it impossible for anyone with
access to the remote bundles to read their contents or to modify them.

zVault uses asymmetric encryption, which means that encryption uses a so called
*public key* and decryption uses a different *secret key*. This makes it
possible to setup a backup configuration where the machine can only create
backups but not read them. Since lots of subcommands need to read the backups,
this setup is not recommended in general.

The key pairs used by zVault can be created by _zvault-genkey(1)_ and added to a
repository via _zvault-addkey(1)_ or upon creation via the `--encryption` flag
in _zvault-init(1)_.

**Important: The key pair is needed to read and restore any encrypted backup.
Loosing the secret key means that all data in the backups is lost forever.
There is no backdoor, even the developers of zVault can not recover a lost key
pair. So it is important to store the key pair in a safe location. The key pair
is small enough to be printed on paper for example.**


### Hash method
ZVault uses hash fingerprints to identify chunks. It is critically important
that no two chunks have the same hash value (a so-called hash collision) as this
would cause one chunk to overwrite the other chunk. For this purpose zVault uses
128 bit hashes, that have a collision probability of less than 1.5e-15 even for
1 trillion stored chunks (about 15.000 TiB stored data in 16 KiB chunks).

ZVault offers two different hash algorithms: **blake2** and **murmur3**.

Murmur3 is blazingly fast but is not cryptographically secure. That means that
while random hash collisions are negligible, an attacker with access to files
could manipulate a file so that it will cause a hash collision and affects other
data in the repository. **This hash should only be used when the security
implications of this are fully understood.**

Blake2 is slower than murmur3 but also pretty fast and this hash algorithm is
cryptographically secure, i.e. even an attacker can not cause hash collisions.

The recommended hash algorithm is **blake2**.



## EXAMPLES

This command will initialize a repository in the default location with
encryption enabled:

    $> zvault init :: -e --remote /mnt/remote/backups

Before using this repository, the key pair located at `~/.zvault/keys` should be
backed up in a safe location (e.g. printed to paper).

This command will create a backup of the whole system tagged by date:

    $> zvault backup / ::system/$(date +%F)

If the home folders are mounted on /home, the following command can be used to
backup them separatly (zVault will not backup mounted folders by default):

    $> zvault backup /home ::homes/$(date +%F)

The backups can be listed by this command:

    $> zvault list ::

and inspected by this command (the date needs to be adapted):

    $> zvault info ::homes/2017-04-06

To restore some files from a backup, the following command can be used:

    $> zvault restore ::homes/2017-04-06::bob/file.dat /tmp

Alternatively the repository can be mounted with this command:

    $> zvault mount ::homes/2017-04-06 /mnt/tmp

A single backup can be removed with this command:

    $> zvault remove ::homes/2017-04-06

Multiple backups can be removed based on their date with the following command
(add `-f` to actually remove backups):

    $> zvault prune :: --prefix system --daily 7 --weekly 5 --monthly 12

To reclaim storage space after removing some backups vacuum needs to be run
(add `-f` to actually remove bundles):

    $> zvault vacuum ::



## COPYRIGHT

Copyright (C) 2017  Dennis Schwerdel
This software is licensed under GPL-3 or newer (see LICENSE.md)
