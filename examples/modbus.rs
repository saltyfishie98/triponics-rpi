#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use tokio_modbus::prelude::*;
    use tokio_serial::SerialStream;

    let tty_path = "/dev/serial0";
    let slave = Slave(0x1);

    let builder = tokio_serial::new(tty_path, 9600);
    let port = SerialStream::open(&builder).unwrap();

    let mut ctx = rtu::attach_slave(port, slave);
    println!("Reading a sensor value");
    let rsp = ctx.read_holding_registers(0x0, 4).await??;
    println!("Sensor value is: {rsp:?}");

    println!("Disconnecting");
    ctx.disconnect().await?.unwrap();

    Ok(())
}
