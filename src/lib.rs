#![forbid(unsafe_code)]

pub mod context;
pub mod ecosystem;
pub mod genome;
pub mod metrics;
pub mod mind;
pub mod run;
pub mod universe;

#[cfg(feature = "viewer")]
pub mod viewer;

#[cfg(test)]
#[path = "../tests/support/mod.rs"]
mod test_support;
