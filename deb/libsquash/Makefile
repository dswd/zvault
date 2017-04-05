undefine LDFLAGS

build:
	git clone https://github.com/quixdb/squash -b 5ea579cae2324f9e814cb3d88aa589dff312e9e2 src
	(cd src; ./autogen.sh --prefix=/usr --disable-external)
	make -C src

clean:
	rm -rf src

install:
	make -C src install DESTDIR=$(DESTDIR)
