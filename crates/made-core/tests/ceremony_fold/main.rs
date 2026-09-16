//! A ceremony is its event stream.
//!
//! Three claims that must fall if broken: for every command, folding
//! what `decide` yields leaves the same session the mutator leaves;
//! after any sequence of mutations, `rehydrate` of the events those
//! mutations decided is the session; and both hold under a few
//! hundred random sequences of accepted and refused commands.

mod decide_apply;
mod fixture;
mod fold_equality;
mod property;
