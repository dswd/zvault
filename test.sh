set -ex

rm -rf repos
mkdir repos
mkdir -p repos/remotes/zvault_brotli3 repos/remotes/zvault_brotli6 repos/remotes/zvault_lzma2
target/release/zvault init --compression brotli/3 --remote $(pwd)/repos/remotes/zvault_brotli3 $(pwd)/repos/zvault_brotli3
target/release/zvault init --compression brotli/6 --remote $(pwd)/repos/remotes/zvault_brotli6 $(pwd)/repos/zvault_brotli6
target/release/zvault init --compression lzma2/2 --remote $(pwd)/repos/remotes/zvault_lzma2 $(pwd)/repos/zvault_lzma2
attic init repos/attic
borg init -e none repos/borg
borg init -e none repos/borg-zlib
zbackup init --non-encrypted repos/zbackup

find test_data/silesia -type f | xargs cat > /dev/null
time target/release/zvault backup test_data/silesia $(pwd)/repos/zvault_brotli3::silesia1
time target/release/zvault backup test_data/silesia $(pwd)/repos/zvault_brotli3::silesia2
time target/release/zvault backup test_data/silesia $(pwd)/repos/zvault_brotli6::silesia1
time target/release/zvault backup test_data/silesia $(pwd)/repos/zvault_brotli6::silesia2
time target/release/zvault backup test_data/silesia $(pwd)/repos/zvault_lzma2::silesia1
time target/release/zvault backup test_data/silesia $(pwd)/repos/zvault_lzma2::silesia2
time attic create repos/attic::silesia1 test_data/silesia
time attic create repos/attic::silesia2 test_data/silesia
time borg create -C none repos/borg::silesia1 test_data/silesia
time borg create -C none repos/borg::silesia2 test_data/silesia
time borg create -C zlib repos/borg-zlib::silesia1 test_data/silesia
time borg create -C zlib repos/borg-zlib::silesia2 test_data/silesia
time tar -c test_data/silesia | zbackup backup --non-encrypted repos/zbackup/backups/silesia1
time tar -c test_data/silesia | zbackup backup --non-encrypted repos/zbackup/backups/silesia2

du -h test_data/silesia.tar
du -sh repos/remotes/zvault* repos/attic repos/borg repos/borg-zlib repos/zbackup

rm -rf repos
mkdir repos
mkdir -p repos/remotes/zvault_brotli3 repos/remotes/zvault_brotli6 repos/remotes/zvault_lzma2
target/release/zvault init --compression brotli/3 --remote $(pwd)/repos/remotes/zvault_brotli3 $(pwd)/repos/zvault_brotli3
target/release/zvault init --compression brotli/6 --remote $(pwd)/repos/remotes/zvault_brotli6 $(pwd)/repos/zvault_brotli6
target/release/zvault init --compression lzma2/2 --remote $(pwd)/repos/remotes/zvault_lzma2 $(pwd)/repos/zvault_lzma2
attic init repos/attic
borg init -e none repos/borg
borg init -e none repos/borg-zlib
zbackup init --non-encrypted repos/zbackup

find test_data/ubuntu -type f | xargs cat > /dev/null
time target/release/zvault backup test_data/ubuntu $(pwd)/repos/zvault_brotli3::ubuntu1
time target/release/zvault backup test_data/ubuntu $(pwd)/repos/zvault_brotli3::ubuntu2
time target/release/zvault backup test_data/ubuntu $(pwd)/repos/zvault_brotli6::ubuntu1
time target/release/zvault backup test_data/ubuntu $(pwd)/repos/zvault_brotli6::ubuntu2
time target/release/zvault backup test_data/ubuntu $(pwd)/repos/zvault_lzma2::ubuntu1
time target/release/zvault backup test_data/ubuntu $(pwd)/repos/zvault_lzma2::ubuntu2
time attic create repos/attic::ubuntu1 test_data/ubuntu
time attic create repos/attic::ubuntu2 test_data/ubuntu
time borg create -C none repos/borg::ubuntu1 test_data/ubuntu
time borg create -C none repos/borg::ubuntu2 test_data/ubuntu
time borg create -C zlib repos/borg-zlib::ubuntu1 test_data/ubuntu
time borg create -C zlib repos/borg-zlib::ubuntu2 test_data/ubuntu
time tar -c test_data/ubuntu | zbackup backup --non-encrypted repos/zbackup/backups/ubuntu1
time tar -c test_data/ubuntu | zbackup backup --non-encrypted repos/zbackup/backups/ubuntu2

du -h test_data/ubuntu.tar
du -sh repos/remotes/zvault* repos/attic repos/borg repos/borg-zlib repos/zbackup
