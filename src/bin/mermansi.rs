//! `mermansi` CLI binary - render Mermaid diagrams to terminal text.

use std::io::{Read, Write};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let stderr = std::io::stderr();
    match run_with_io(
        &args,
        &mut stdin.lock(),
        &mut stdout.lock(),
        &mut stderr.lock(),
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => ExitCode::from(code),
    }
}

fn run_with_io(
    args: &[String],
    stdin: &mut impl Read,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> Result<(), u8> {
    let mut file: Option<&str> = None;
    let mut charset_unicode = true;
    let mut color_mode = mermansi::ColorMode::Plain;
    let mut output_mode = mermansi::OutputMode::Complete;
    let mut max_width: Option<usize> = None;
    let mut max_height: Option<usize> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--help" | "-h" => {
                return write_help(stdout).map_err(|error| report_io(stderr, "stdout", error));
            }
            "--version" | "-V" => {
                return writeln!(stdout, "mermansi {}", env!("CARGO_PKG_VERSION"))
                    .map_err(|error| report_io(stderr, "stdout", error));
            }
            "--ascii" => charset_unicode = false,
            "--unicode" => charset_unicode = true,
            "--color" => color_mode = mermansi::ColorMode::Ansi16,
            "--truecolor" => color_mode = mermansi::ColorMode::TrueColor,
            "--no-color" => color_mode = mermansi::ColorMode::Plain,
            "--concise" => output_mode = mermansi::OutputMode::Concise,
            "--complete" => output_mode = mermansi::OutputMode::Complete,
            "--file" => {
                i += 1;
                if i >= args.len() {
                    return fail(stderr, 3, "--file requires a path argument");
                }
                if file.is_some() {
                    return fail(stderr, 3, "--file may only be specified once");
                }
                file = Some(&args[i]);
            }
            "--width" => {
                i += 1;
                if i >= args.len() {
                    return fail(stderr, 3, "--width requires a numeric argument");
                }
                max_width = Some(parse_dimension("--width", &args[i], stderr)?);
            }
            "--height" => {
                i += 1;
                if i >= args.len() {
                    return fail(stderr, 3, "--height requires a numeric argument");
                }
                max_height = Some(parse_dimension("--height", &args[i], stderr)?);
            }
            arg if arg.starts_with("--file=") => {
                if file.is_some() {
                    return fail(stderr, 3, "--file may only be specified once");
                }
                if arg.len() == "--file=".len() {
                    return fail(stderr, 3, "--file requires a path argument");
                }
                file = Some(&arg[7..]);
            }
            arg if arg.starts_with("--width=") => {
                max_width = Some(parse_dimension("--width", &arg[8..], stderr)?);
            }
            arg if arg.starts_with("--height=") => {
                max_height = Some(parse_dimension("--height", &arg[9..], stderr)?);
            }
            other => return fail(stderr, 3, &format!("unknown argument '{other}'")),
        }
        i += 1;
    }

    let source = match file {
        Some(path) => {
            let mut input = std::fs::File::open(path).map_err(|error| {
                report(stderr, 3, &format!("cannot open file '{path}': {error}"))
            })?;
            read_source(&mut input).map_err(|error| {
                report(
                    stderr,
                    error.code(),
                    &format!("cannot read file '{path}': {error}"),
                )
            })?
        }
        None => read_source(stdin).map_err(|error| {
            report(stderr, error.code(), &format!("cannot read stdin: {error}"))
        })?,
    };

    let mut opts = if charset_unicode {
        mermansi::MermansiOptions::unicode()
    } else {
        mermansi::MermansiOptions::ascii()
    };
    opts = opts.with_color(color_mode).with_output_mode(output_mode);
    if let Some(width) = max_width {
        opts = opts.with_max_width(width);
    }
    if let Some(height) = max_height {
        opts = opts.with_max_height(height);
    }

    match mermansi::render_source(&source, &opts) {
        Ok(output) => {
            stdout
                .write_all(output.as_bytes())
                .and_then(|()| stdout.flush())
                .map_err(|error| report_io(stderr, "stdout", error))?;
            Ok(())
        }
        Err(mermansi::MermansiError::Parse(error)) => {
            fail(stderr, 1, &format!("parse error: {error}"))
        }
        Err(mermansi::MermansiError::JsonSource { source }) => {
            fail(stderr, 1, &format!("parse error: {source}"))
        }
        Err(mermansi::MermansiError::InvalidOption { field, message }) => {
            fail(stderr, 3, &format!("invalid option '{field}': {message}"))
        }
        Err(error) => fail(stderr, 2, &format!("render error: {error}")),
    }
}

fn parse_dimension(name: &str, value: &str, stderr: &mut impl Write) -> Result<usize, u8> {
    let parsed = value
        .parse::<usize>()
        .map_err(|_| report(stderr, 3, &format!("{name} must be a positive integer")))?;
    if parsed == 0 {
        return fail(stderr, 3, &format!("{name} must be greater than zero"));
    }
    Ok(parsed)
}

#[derive(Debug)]
enum SourceReadError {
    Io(std::io::Error),
    TooLarge,
}

impl SourceReadError {
    fn code(&self) -> u8 {
        match self {
            Self::Io(_) => 3,
            Self::TooLarge => 2,
        }
    }
}

impl std::fmt::Display for SourceReadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => error.fmt(formatter),
            Self::TooLarge => write!(
                formatter,
                "source exceeds {} byte limit",
                mermansi::MAX_SOURCE_BYTES
            ),
        }
    }
}

fn read_source(reader: &mut impl Read) -> Result<String, SourceReadError> {
    let limit = u64::try_from(mermansi::MAX_SOURCE_BYTES)
        .unwrap_or(u64::MAX)
        .saturating_add(1);
    let mut bytes = Vec::with_capacity(8 * 1024);
    reader
        .take(limit)
        .read_to_end(&mut bytes)
        .map_err(SourceReadError::Io)?;
    if bytes.len() > mermansi::MAX_SOURCE_BYTES {
        return Err(SourceReadError::TooLarge);
    }
    String::from_utf8(bytes).map_err(|error| {
        SourceReadError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, error))
    })
}

fn report(stderr: &mut impl Write, code: u8, message: &str) -> u8 {
    let _ = writeln!(stderr, "error: {message}");
    code
}

fn report_io(stderr: &mut impl Write, stream: &str, error: std::io::Error) -> u8 {
    report(stderr, 3, &format!("cannot write {stream}: {error}"))
}

fn fail<T>(stderr: &mut impl Write, code: u8, message: &str) -> Result<T, u8> {
    Err(report(stderr, code, message))
}

fn write_help(output: &mut impl Write) -> std::io::Result<()> {
    writeln!(
        output,
        "mermansi {version} - Mermaid terminal renderer\n\
\n\
USAGE:\n    mermansi [OPTIONS]\n\
\n\
OPTIONS:\n    --file <PATH>      Read Mermaid source from a file (default: stdin)\n    --ascii            Use ASCII charset\n    --unicode          Use Unicode charset (default)\n    --color            Enable ANSI 16-color roles\n    --truecolor        Enable 24-bit ANSI color roles\n    --no-color         Disable ANSI color (default)\n    --concise          Emit terminal geometry only\n    --complete         Emit geometry plus canonical model (default)\n    --width <N>        Maximum terminal-preview width in columns\n    --height <N>       Maximum terminal-preview height in rows\n    --version, -V      Print version and exit\n    --help, -h         Print this help and exit\n\
\n\
EXIT CODES:\n    0  Success\n    1  Parse error\n    2  Render error\n    3  Invalid options or I/O error",
        version = env!("CARGO_PKG_VERSION")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FailingWriter;

    impl Write for FailingWriter {
        fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "closed",
            ))
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn rejects_zero_width_as_invalid_option() {
        let mut output = Vec::new();
        let mut errors = Vec::new();
        let result = run_with_io(
            &args(&["mermansi", "--width", "0"]),
            &mut "flowchart TD\nA-->B".as_bytes(),
            &mut output,
            &mut errors,
        );
        assert_eq!(result, Err(3));
        assert!(
            String::from_utf8(errors)
                .unwrap()
                .contains("greater than zero")
        );
    }

    #[test]
    fn reports_stdout_failures() {
        let mut errors = Vec::new();
        let result = run_with_io(
            &args(&["mermansi"]),
            &mut "flowchart TD\nA-->B".as_bytes(),
            &mut FailingWriter,
            &mut errors,
        );
        assert_eq!(result, Err(3));
        assert!(
            String::from_utf8(errors)
                .unwrap()
                .contains("cannot write stdout")
        );
    }

    #[test]
    fn bounds_stdin_before_rendering() {
        let input = vec![b'x'; mermansi::MAX_SOURCE_BYTES + 1];
        let mut output = Vec::new();
        let mut errors = Vec::new();
        let result = run_with_io(
            &args(&["mermansi"]),
            &mut input.as_slice(),
            &mut output,
            &mut errors,
        );
        assert_eq!(result, Err(2));
        assert!(String::from_utf8(errors).unwrap().contains("byte limit"));
    }

    #[test]
    fn truecolor_is_emitted_without_changing_content() {
        let source = "pie\n\"A\" : 1";
        let mut output = Vec::new();
        let mut errors = Vec::new();
        let result = run_with_io(
            &args(&["mermansi", "--truecolor"]),
            &mut source.as_bytes(),
            &mut output,
            &mut errors,
        );
        assert_eq!(result, Ok(()));
        assert!(output.windows(7).any(|window| window == b"\x1b[38;2;"));
        assert!(errors.is_empty());
    }
}
