#[derive(serde::Serialize)]
pub struct Device {
    pub identifiers: &'static [&'static str],
    pub name: &'static str,
}
