use std::fs::OpenOptions;
use std::io::{stdout, Write};
use std::{io, iter};

pub(crate) fn set_summary<M: Into<String>>(markdown: M) -> Result<(), WriteActionDataError> {
    let markdown = markdown.into();
    write_data("GITHUB_STEP_SUMMARY", format!("{markdown}\n").as_bytes())
}

pub(crate) fn set_output<N: Into<String>, V: Into<String>>(
    name: N,
    value: V,
) -> Result<(), WriteActionDataError> {
    let name = name.into();
    let value = value.into();
    let line = if value.contains('\n') {
        let delimiter: String = iter::repeat_with(fastrand::alphanumeric).take(20).collect();
        format!("{name}<<{delimiter}\n{value}\n{delimiter}")
    } else {
        format!("{name}={value}")
    };
    let line = format!("{line}\n");
    write_data("GITHUB_OUTPUT", line.as_bytes())
}

fn write_data(env_name: &str, data: &[u8]) -> Result<(), WriteActionDataError> {
    let mut file: Box<dyn Write> = match std::env::var(env_name) {
        Ok(github_output) => {
            let append_file = OpenOptions::new()
                .append(true)
                .open(github_output)
                .map_err(WriteActionDataError::Opening)?;
            Box::new(append_file)
        }
        Err(_) => Box::new(stdout()),
    };

    file.write_all(data).map_err(WriteActionDataError::Writing)
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum WriteActionDataError {
    #[error("Could not open action data file\nError: {0}")]
    Opening(#[source] io::Error),
    #[error("Could not write action data file\nError: {0}")]
    Writing(#[source] io::Error),
}
