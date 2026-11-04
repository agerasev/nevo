// Serde's built-in array implementations stop at length 32.
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error};

pub fn serialize<S: Serializer>(weights: &[f32; 47], serializer: S) -> Result<S::Ok, S::Error> {
    weights.as_slice().serialize(serializer)
}

pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<[f32; 47], D::Error> {
    let weights = Vec::<f32>::deserialize(deserializer)?;
    weights.try_into().map_err(|weights: Vec<f32>| {
        D::Error::custom(format!("expected 47 weights, got {}", weights.len()))
    })
}
