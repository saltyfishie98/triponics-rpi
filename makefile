rpi_addr=10.42.0.11
proj_name=$(shell basename $(CURDIR))

build-debug: 
	cross build --target aarch64-unknown-linux-gnu
send-debug: 
	rsync -avz --mkpath target/aarch64-unknown-linux-gnu/debug/$(proj_name) saltyfishie@$(rpi_addr):~/Programs/debug/
debug-killall: 
	ssh saltyfishie@$(rpi_addr) killall lldb-server $(proj_name); echo 0
debug: build-debug debug-killall 
	ssh saltyfishie@$(rpi_addr) "sh -c 'nohup lldb-server platform --server --listen *:17777' > /dev/null 2>&1 &"

build-release: 
	cross build --target aarch64-unknown-linux-gnu --release
send-release:
	rsync -avz --mkpath target/aarch64-unknown-linux-gnu/release/$(proj_name) saltyfishie@$(rpi_addr):~/Programs/triponics-rpi/
dist: build-release send-release
