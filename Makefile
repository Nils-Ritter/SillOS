KERNEL := target/x86_64-unknown-none/debug/sillos
ISO := sillos.iso
TEST_ISO := sillos_tests.iso
LIMINE := limine/limine
LIMINE_DIR := limine

.PHONY: all clean run limine kernel

all: $(ISO)

$(LIMINE):
	$(MAKE) -C $(LIMINE_DIR)

kernel:
	cargo build

kernel-tests:
	cargo build --features test

$(ISO): kernel $(LIMINE)
	rm -rf iso_root
	mkdir -p iso_root/boot
	mkdir -p iso_root/EFI/BOOT

	cp $(KERNEL) iso_root/boot/sillos
	cp limine.conf iso_root/limine.conf

	cp limine/limine-bios-cd.bin iso_root/boot/
	cp limine/limine-uefi-cd.bin iso_root/boot/
	cp limine/BOOTX64.EFI iso_root/EFI/BOOT/
	cp limine/limine-bios.sys iso_root/boot/

	xorriso -as mkisofs \
		-R -r -J \
		-b boot/limine-bios-cd.bin \
		-no-emul-boot \
		-boot-load-size 4 \
		-boot-info-table \
		--efi-boot boot/limine-uefi-cd.bin \
		-efi-boot-part \
		--efi-boot-image \
		--protective-msdos-label \
		iso_root \
		-o $(ISO)

	$(LIMINE) bios-install $(ISO)

$(TEST_ISO): kernel-tests $(LIMINE)
	rm -rf iso_root
	mkdir -p iso_root/boot
	mkdir -p iso_root/EFI/BOOT

	cp $(KERNEL) iso_root/boot/sillos
	cp limine.conf iso_root/limine.conf

	cp limine/limine-bios-cd.bin iso_root/boot/
	cp limine/limine-uefi-cd.bin iso_root/boot/
	cp limine/BOOTX64.EFI iso_root/EFI/BOOT/
	cp limine/limine-bios.sys iso_root/boot/

	xorriso -as mkisofs \
		-R -r -J \
		-b boot/limine-bios-cd.bin \
		-no-emul-boot \
		-boot-load-size 4 \
		-boot-info-table \
		--efi-boot boot/limine-uefi-cd.bin \
		-efi-boot-part \
		--efi-boot-image \
		--protective-msdos-label \
		iso_root \
		-o $(TEST_ISO)

	$(LIMINE) bios-install $(TEST_ISO)

run: $(ISO)
	qemu-system-x86_64 -cdrom $(ISO) -m 256M

test: $(TEST_ISO)
	qemu-system-x86_64 -cdrom $(TEST_ISO) -m 256M -display none -serial stdio -device isa-debug-exit,iobase=0xf4,iosize=0x04

clean:
	cargo clean
	rm -rf iso_root $(ISO)
	rm -rf iso_root $(TEST_ISO)
	rm -rf $(LIMINE)

cleaniso:
	rm -rf iso_root $(ISO)

limine: $(LIMINE)
