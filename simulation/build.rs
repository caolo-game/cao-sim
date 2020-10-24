#![allow(unused_imports)]

use std::env::var;
use std::fs::{read_dir, File, OpenOptions};
use std::io::prelude::*;

#[cfg(not(feature = "mp_executor"))]
fn main() {}

#[cfg(feature = "mp_executor")]
fn main() {
    let file_list = read_dir("cpnp")
        .expect("capnp dir list")
        .filter_map(|f| f.ok())
        .map(|f| f.path());

    let mut compiler = ::capnpc::CompilerCommand::new();

    for file in file_list {
        compiler.file(file);
    }

    compiler.run().expect("compiling schema");
}
