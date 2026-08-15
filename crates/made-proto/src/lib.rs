//! Generated gRPC bindings for MADE.
//!
//! Wire contract: `underpass.made.v1`.

#![allow(clippy::pedantic)]
#![allow(clippy::all)]

pub mod v1 {
    tonic::include_proto!("underpass.made.v1");
}

pub mod runtime_v1 {
    tonic::include_proto!("underpass.runtime.v1");
}

pub use v1::*;
