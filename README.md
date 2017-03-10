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
level. Backups can be delete and chunks that are not used by any backup can be
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

- Use rolling checksum to create content-dependent chunks
- Use sha3-shake128 to hash chunks
- Use mmapped hashtable to find duplicate chunks
- Serialize metadata into chunks
- Store small file data within metadata
- Store directory metadata to avoid calculating checksums of unchanged files (same mtime and size)
- Store full directory tree in each backup (use cached metadata and checksums for unchanged entries)
- Compress data chunks in blocks of ~10MB to improve compression ("solid archive")
- Store metadata in separate data chunks to enable metadata caching on client
- Encrypt archive
- Sort new files by file extension to improve compression

## Configurable parameters

- Rolling chunker algorithm
- Minimal chunk size [default: 1 KiB]
- Maximal chunk size [default: 64 KiB]
- Maximal file size for inlining [default: 128 Bytes]
- Block size [default: 10 MiB]
- Block compression algorithm [default: Brotli 6]
- Encryption algorithm [default: chacha20+poly1305]

## TODO

- Remove old data
- Locking / Multiple clients

## Modules

- Rolling checksum chunker
  - Also creates hashes
- Mmapped hashtable that stores existing chunks hashes
- Remote block writing and compression/encryption
- Inode data serialization
- Recursive directory scanning, difference calculation, new entry sorting


### ChunkDB

- Stores data in chunks
- A chunk is a file
- Per Chunk properties
  - Format version
  - Encryption method
  - Encryption key
  - Compression method / level
- Chunk ID is the hash of the contents
  - No locks needed on shared chunk repository !!!
  - Chunk ID is calculated after compression and encryption
- Chunk header
  - "zvault01"
  - Chunk size compressed / raw
  - Content hash method / value
  - Encryption method / options / key hash
  - Compression method / options
- Chunks are write-once read-often
- Chunks are prepared outside the repository
- Only one chunk is being prepared at a time
- Adding data to the chunk returns starting position in raw data
- Operations:
  - List available chunks
  - Add data
  - Flush chunk
  - Delete chunk
  - Get data
  - Check chunk
- Chunk path is `checksum.chunk` or `chec/ksum.chunk`
- Data is added to current chunk and compressed in memory
- Operations on chunk files are just sequencial read/write and delete
- Ability to recompress chunks


### Index

16 Bytes per hash key
8 Bytes data per entry (4 bytes bundle id, 4 bytes chunk id)
=> 24 Bytes per entry

Average chunk sizes
 8 Kib => 3 MiB / 1 GiB
16 Kib => 1.5 MiB / 1 GiB
24 Kib => 1.0 MiB / 1 GiB
32 Kib => 750 Kib / 1 GiB
64 Kib => 375 Kib / 1 GiB
