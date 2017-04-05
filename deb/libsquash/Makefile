undefine LDFLAGS

build:
	(cd src; ./autogen.sh --prefix=/usr)
	make -C src

clean:
	rm -rf src
	git clone https://github.com/quixdb/squash src

install:
	make -C src install DESTDIR=$(DESTDIR)
