#[macro_use]
extern crate serde_derive;

pub mod models {
	include!(concat!(env!("OUT_DIR"), "/generated/src/models/mod.rs"));
}
