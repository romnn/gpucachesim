#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
    clippy::missing_panics_doc,
    clippy::missing_errors_doc
)]
// #![allow(warnings)]

pub mod matrixmul;
pub mod simple_matrixmul;
pub mod transpose;
pub mod vectoradd;

pub type Result = color_eyre::eyre::Result<(
    Vec<trace_model::command::Command>,
    Vec<(
        trace_model::command::KernelLaunch,
        trace_model::MemAccessTrace,
    )>,
)>;
