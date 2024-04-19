#[macro_use]
extern crate serde_derive;

pub mod models {
	include!(concat!(env!("OUT_DIR"), "/generated/src/models/mod.rs"));
}

#[cfg(feature = "client")]
pub mod apis {
	include!(concat!(env!("OUT_DIR"), "/generated/src/apis/mod.rs"));
}
