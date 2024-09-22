fn main() {
    let ports = serialport::available_ports().expect("No ports found!");
    for p in ports {
        println!("{}", p.port_name);
    }

    let port = serialport::new("/dev/ttyUSB0", 115_200)
        .timeout(std::time::Duration::from_millis(10))
        .open()
        .expect("Failed to open port");
}
