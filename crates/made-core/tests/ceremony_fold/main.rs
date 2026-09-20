//! A ceremony is its event stream.
//!
//! Four claims that must fall if broken: for every command, folding
//! what `decide` yields leaves the same session the mutator leaves;
//! after any sequence of mutations, `rehydrate` of the events those
//! mutations decided is the session; both hold under a few hundred
//! random sequences of accepted and refused commands; and a session
//! imported from a pre-stream store folds to the snapshot it carried.

mod budget_tree;
mod decide_apply;
mod fixture;
mod fold_equality;
mod import;
mod lifecycle;
mod property;
mod succession;
