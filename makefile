build-debug: 
	cross build --target aarch64-unknown-linux-gnu
send-debug:
	rsync -avz target/aarch64-unknown-linux-gnu/debug/$(shell basename $(CURDIR))	 saltyfishie@169.254.0.100:/home/saltyfishie/Programs/
debug: build-debug send-debug

build-release: 
	cross build --target aarch64-unknown-linux-gnu --release
send-release:
	rsync -avz target/aarch64-unknown-linux-gnu/release/$(shell basename $(CURDIR))	 saltyfishie@169.254.0.100:/home/saltyfishie/Programs/
dist: build-release send-release
