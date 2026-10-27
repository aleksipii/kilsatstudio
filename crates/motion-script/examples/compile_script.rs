//! Compile a v1 schedule from stdin to legacy editable YAML on stdout.
use std::io::{self, Read};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut source = String::new();
    io::stdin()
        .take((motion_script::compiler::MAX_SOURCE_BYTES + 1) as u64)
        .read_to_string(&mut source)?;
    let script = motion_script::compiler::compile(&source)?;
    print!("{}", serde_yaml::to_string(&script)?);
    Ok(())
}
