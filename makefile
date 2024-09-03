addr=10.42.0.11

build-debug: 
	cross build --target aarch64-unknown-linux-gnu
send-debug:
	rsync -avz --mkpath target/aarch64-unknown-linux-gnu/debug/$(shell basename $(CURDIR)) saltyfishie@$(addr):~/Programs/debug/
debug: build-debug send-debug

build-release: 
	cross build --target aarch64-unknown-linux-gnu --release
send-release:
	rsync -avz --mkpath target/aarch64-unknown-linux-gnu/release/$(shell basename $(CURDIR)) saltyfishie@$(addr):~/Programs/triponics-rpi/
dist: build-release send-release
