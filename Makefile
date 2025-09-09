IMAGE_NAME := kernel


limine/limine:
	rm -rf limine
	git clone https://github.com/limine-bootloader/limine.git --branch=v9.x-binary --depth=1
	$(MAKE) -C limine
ovmf/ovmf-code.fd:
	mkdir -p ovmf
	curl -Lo $@ https://github.com/osdev0/edk2-ovmf-nightly/releases/latest/download/ovmf-code-x86_64.fd

ovmf/ovmf-vars.fd:
	mkdir -p ovmf
	curl -Lo $@ https://github.com/osdev0/edk2-ovmf-nightly/releases/latest/download/ovmf-vars-x86_64.fd

$(IMAGE_NAME).iso: limine/limine $(BIN_PATH)
	rm -rf iso_root
	mkdir -p iso_root/boot
	cp -v $(BIN_PATH) iso_root/boot/kernel
	nm --demangle iso_root/boot/kernel -n > iso_root/boot/kernel.symbols
	mkdir -p iso_root/boot/limine
	cp -v limine.conf iso_root/boot/limine/
	mkdir -p iso_root/EFI/BOOT
	cp -v limine/limine-bios.sys limine/limine-bios-cd.bin limine/limine-uefi-cd.bin iso_root/boot/limine/
	cp -v limine/BOOTX64.EFI iso_root/EFI/BOOT/
	cp -v limine/BOOTIA32.EFI iso_root/EFI/BOOT/
	xorriso -as mkisofs -b boot/limine/limine-bios-cd.bin \
		-no-emul-boot -boot-load-size 4 -boot-info-table \
		--efi-boot boot/limine/limine-uefi-cd.bin \
		-efi-boot-part --efi-boot-image --protective-msdos-label \
		iso_root -o $(IMAGE_NAME).iso
	./limine/limine bios-install $(IMAGE_NAME).iso
	rm -rf iso_root


.PHONY: qemu
qemu: $(IMAGE_NAME).iso ovmf/ovmf-code.fd ovmf/ovmf-vars.fd $(BIN_PATH)
	qemu-system-x86_64 -cdrom kernel.iso -debugcon stdio -smp 4 -m 1G \
		-drive if=pflash,unit=0,format=raw,file=ovmf/ovmf-code.fd,readonly=on \
		-drive if=pflash,unit=1,format=raw,file=ovmf/ovmf-vars.fd $(QEMU_ARGS) || true

.PHONY: clean
clean: 
	rm -rf ovmf limine kernel.iso iso_root