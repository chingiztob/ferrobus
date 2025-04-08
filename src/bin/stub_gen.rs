#[cfg(feature = "stubgen")]
use pyo3_stub_gen::Result;
#[cfg(feature = "stubgen")]
fn main() -> Result<()> {
    // `stub_info` is a function defined by `define_stub_info_gatherer!` macro.
    let stub = ferrobus::stub_info()?;
    stub.generate()?;
    Ok(())
}

#[cfg(not(feature = "stubgen"))]
fn main() {
    eprintln!("The 'stubgen' feature is not enabled. Enable it with `--features stubgen`.");
}
