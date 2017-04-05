build: zvault.1

zvault.1: zvault.1.ronn
	ronn -r zvault.1.ronn

install:
	install -d $(DESTDIR)/usr/bin
	install -m 755 zvault $(DESTDIR)/usr/bin/zvault
