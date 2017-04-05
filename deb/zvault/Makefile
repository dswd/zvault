build: man/zvault.1 man/zvault-backup.1 man/zvault-check.1 man/zvault-config.1 \
	   man/zvault-import.1 man/zvault-info.1 man/zvault-init.1 man/zvault-list.1 \
	   man/zvault-mount.1 man/zvault-prune.1 man/zvault-remove.1 \
	   man/zvault-restore.1 man/zvault-vacuum.1

%.1: %.1.md
	ronn -r $<

install:
	install -d $(DESTDIR)/usr/bin
	install -m 755 zvault $(DESTDIR)/usr/bin/zvault
