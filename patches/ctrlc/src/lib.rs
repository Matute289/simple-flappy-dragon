// Stub implementation of ctrlc for wasm32 compatibility.
// bracket-terminal 0.8.2 depends on ctrlc unconditionally but only uses it in
// the crossterm backend, which is never compiled for wasm32.

#[derive(Debug, Clone, PartialEq)]
pub struct Signal(i32);

#[derive(Debug)]
pub enum Error {
    NoSuchSignal(Signal),
    MultipleHandlers,
    System(std::io::Error),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ctrlc error")
    }
}

impl std::error::Error for Error {}

pub fn set_handler<F>(_handler: F) -> Result<(), Error>
where
    F: Fn() + 'static + Send,
{
    Ok(())
}
