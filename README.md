# ZVault Backup solution

## Goals / Features


### Space-efficient storage with deduplication
The backup data is split into chunks. Fingerprints make sure that each chunk is
only stored once. The chunking algorithm is designed so that small changes to a
file only change a few chunks and leave most chunks unchanged.

Multiple backups of the same data set will only take up the space of one copy.

The chunks are combined into bundles. Each bundle holds chunks up to a maximum
data size and is compressed as a whole to save space ("solid archive").


### Independent backups
All backups share common data in form of chunks but are independent on a higher
level. Backups can be deleted and chunks that are not used by any backup can be
removed.

Other backup solutions use differential backups organized in chains. This makes
those backups dependent on previous backups in the chain, so that those backups
can not be deleted. Also, restoring chained backups is much less efficient.


### Fast backup runs
* Only adding changed files
* In-Memory Hashtable


### Backup verification
* Bundles verification
* Index verification
* File structure verification



## Configuration options
There are several configuration options with trade-offs attached so these are
exposed to users.


### Chunker algorithm
The chunker algorithm is responsible for splitting files into chunks in a way
that survives small changes to the file so that small changes still yield
many matching chunks. The quality of the algorithm affects the deduplication
rate and its speed affects the backup speed.

There are 3 algorithms to choose from:

The **Rabin chunker** is a very common algorithm with a good quality but a
mediocre speed (about 350 MB/s).
The **AE chunker** is a novel approach that can reach very high speeds
(over 750 MB/s) but at a cost of quality.
The **FastCDC** algorithm has a slightly higher quality than the Rabin chunker
and is quite fast (about 550 MB/s).

The recommendation is **FastCDC**.


### Chunk size
The chunk size determines the memory usage during backup runs. For every chunk
in the backup repository, 24 bytes of memory are needed. That means that for
every GiB stored in the repository the following amount of memory is needed:
- 8 KiB chunks => 3 MiB / GiB
- 16 KiB chunks => 1.5 MiB / GiB
- 32 KiB chunks => 750 KiB / GiB
- 64 KiB chunks => 375 KiB / GiB

On the other hand, bigger chunks reduce the deduplication efficiency. Even small
changes of only one byte will result in at least one complete chunk changing.


### Hash algorithm
Blake2
Murmur3

Recommended: Blake2


### Bundle size
10 M
25 M
100 M

Recommended: 25 MiB


### Compression

Recommended: Brotli/2-7


## Design


## TODO

### Core functionality
- Recompress & combine bundles
- Allow to use tar files for backup and restore (--tar, http://alexcrichton.com/tar-rs/tar/index.html)
- File attributes
  - xattrs https://crates.io/crates/xattr

### Formats
- Bundles
  - Encrypted bundle header
  - Random bundle name
- Metadata
  - Arbitrarily nested chunk lists
  - Cumulative size, chunk count, dir/file count
- Permissive msgpack mode

### CLI functionality
- list --tree

### Other
- Stability
- Tests & benchmarks
  - Chunker
  - Index
  - BundleDB
  - Bundle map
  - Config files
  - Backup files
  - Backup
  - Prune
  - Vacuum
- Documentation
  - All file formats
  - Design
