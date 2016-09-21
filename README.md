# ZVault Backup solution

## Goals

- Blazingly fast backup runs
- Space-efficient storage
- Independent backups

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
