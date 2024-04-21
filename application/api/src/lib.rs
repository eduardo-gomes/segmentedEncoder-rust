#[macro_use]
extern crate serde_derive;

#[allow(unused_imports)]
pub mod models {
	include!(concat!(env!("OUT_DIR"), "/generated/src/models/mod.rs"));
}

#[cfg(feature = "client")]
#[allow(unused_imports)]
pub mod apis {
	include!(concat!(env!("OUT_DIR"), "/generated/src/apis/mod.rs"));
}
