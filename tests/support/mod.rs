use std::path::PathBuf;

pub fn preset(name: &str) -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("configs")
        .join(format!("{name}.toml"))
        .to_str()
        .unwrap()
        .to_owned()
}

pub fn ecosystem<T: serde::de::DeserializeOwned>(name: &str) -> T {
    let text = std::fs::read_to_string(preset(name)).unwrap();
    let document: toml::Value = toml::from_str(&text).unwrap();
    document["ecosystem"].clone().try_into().unwrap()
}
