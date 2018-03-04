
## Silesia corpus

| Tool           | 1st run | 2nd run | Repo Size |
| -------------- | -------:| -------:| ---------:|
| zvault/brotli3 |    4.0s |    0.0s |    65 MiB |
| zvault/brotli6 |   16.1s |    0.0s |    58 MiB |
| zvault/lzma2   |   45.1s |    0.0s |    55 MiB |
| attic          |   12.7s |    0.3s |    68 MiB |
| borg           |    4.2s |    0.5s |   203 MiB |
| borg/zlib      |   13.2s |    0.4s |    66 MiB |
| zbackup        |   52.3s |    2.0s |    52 MiB |


## Ubuntu 16.04 docker image

| Tool           | 1st run | 2nd run | Repo Size |
| -------------- | -------:| -------:| ---------:|
| zvault/brotli3 |    2.0s |    0.1s |    30 MiB |
| zvault/brotli6 |    7.6s |    0.1s |    25 MiB |
| zvault/lzma2   |   17.6s |    0.1s |    22 MiB |
| attic          |    6.9s |    0.6s |    35 MiB |
| borg           |    3.0s |    0.9s |    83 MiB |
| borg/zlib      |    7.9s |    1.0s |    36 MiB |
| zbackup        |   17.2s |    1.0s |    24 MiB |
