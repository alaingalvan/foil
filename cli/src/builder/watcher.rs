use super::build_mode::BuildMode;

use crate::error::Result;

use std::io::{Write, stdout};

pub async fn watch(_build_mode: BuildMode) -> Result<()> {
    let mut out = stdout();
    out.write(b"Watch mode is currently not implemented.")?;
    Ok(())
}