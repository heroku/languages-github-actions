use rand::distributions::{Alphanumeric, DistString};
use std::fs::OpenOptions;
use std::io;
use std::io::{stdout, Write};

pub(crate) fn set_output<N: Into<String>, V: Into<String>>(
    name: N,
    value: V,
) -> Result<(), SetActionOutputError> {
    let name = name.into();
    let value = value.into();

    let line = if value.contains('\n') {
        let delimiter = Alphanumeric.sample_string(&mut rand::thread_rng(), 20);
        format!("{name}<<{delimiter}\n{value}\n{delimiter}")
    } else {
        format!("{name}={value}")
    };
    let line = format!("{line}\n");

    let mut file: Box<dyn Write> = match std::env::var("GITHUB_OUTPUT") {
        Ok(github_output) => {
            let append_file = OpenOptions::new()
                .append(true)
                .open(github_output)
                .map_err(SetActionOutputError::Opening)?;
            Box::new(append_file)
        }
        Err(_) => Box::new(stdout()),
    };

    file.write_all(line.as_bytes())
        .map_err(SetActionOutputError::Writing)
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum SetActionOutputError {
    #[error("Could not open action output\nError: {0}")]
    Opening(#[source] io::Error),
    #[error("Could not write action output\nError: {0}")]
    Writing(#[source] io::Error),
}
