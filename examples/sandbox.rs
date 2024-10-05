fn main() {
    let fmt = time::macros::format_description!("[hour]:[minute]:[second].[subsecond digits:3]");

    let time = time::Time::parse("05:05:05.000", fmt).unwrap();

    println!("{time}")
}
