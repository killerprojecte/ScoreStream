use scorestream_formats::{ScoreAdapter, musicxml::MusicXml, sfs::SfsAdapter};
use std::{env, fs, io::Read, process::ExitCode};
fn main() -> ExitCode {
    let args: Vec<_> = env::args().collect();
    if args
        .get(1)
        .is_some_and(|v| v == "convert" || v == "inspect-format")
    {
        return conversion(&args);
    }
    if args.len() != 3 || !["validate", "inspect", "canonical"].contains(&args[1].as_str()) {
        usage();
        return ExitCode::from(2);
    }
    let source = match fs::read_to_string(&args[2]) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    match scorestream_codec::parse(&source) {
        Ok(doc) => {
            match args[1].as_str() {
                "inspect" => println!("{}", serde_json::to_string_pretty(&doc).unwrap()),
                "canonical" => print!("{}", scorestream_codec::canonical(&doc)),
                _ => println!("Valid ScoreStream 1.1 score"),
            };
            ExitCode::SUCCESS
        }
        Err(d) => {
            eprintln!("{}", serde_json::to_string_pretty(&d).unwrap());
            ExitCode::FAILURE
        }
    }
}

fn usage() {
    eprintln!(
        "Usage:\n  scorestream <validate|inspect|canonical> <score.sfs>\n  scorestream inspect-format <sfs|musicxml> <input>\n  scorestream convert <sfs|musicxml> <sfs|musicxml> <input>\nConverted text goes to stdout; structured diagnostics go to stderr. Unsupported mappings produce no output."
    );
}
fn adapter(name: &str) -> Option<Box<dyn ScoreAdapter>> {
    match name {
        "sfs" => Some(Box::new(SfsAdapter)),
        "musicxml" => Some(Box::new(MusicXml)),
        _ => None,
    }
}
fn conversion(args: &[String]) -> ExitCode {
    let inspect = args[1] == "inspect-format";
    if args.len() != if inspect { 4 } else { 5 } {
        usage();
        return ExitCode::from(2);
    }
    let Some(input) = adapter(&args[2]) else {
        usage();
        return ExitCode::from(2);
    };
    let output = if inspect {
        None
    } else {
        match adapter(&args[3]) {
            Some(a) => Some(a),
            None => {
                usage();
                return ExitCode::from(2);
            }
        }
    };
    let path = &args[if inspect { 3 } else { 4 }];
    let mut source = String::new();
    match fs::File::open(path).and_then(|f| {
        f.take((scorestream_formats::musicxml::MAX_BYTES + 1) as u64)
            .read_to_string(&mut source)
    }) {
        Ok(_) => {}
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    }
    if source.len() > scorestream_formats::musicxml::MAX_BYTES {
        eprintln!("Conversion input exceeds 8 MiB");
        return ExitCode::FAILURE;
    }
    let converted = match input.read(&source) {
        Ok(v) => v,
        Err(r) => {
            eprintln!("{}", serde_json::to_string_pretty(&r).unwrap());
            return ExitCode::FAILURE;
        }
    };
    let mut report = converted.report;
    let text = if let Some(output) = output {
        match output.write(&converted.value) {
            Ok(result) => {
                report.diagnostics.extend(result.report.diagnostics);
                result.value
            }
            Err(result) => {
                report.diagnostics.extend(result.diagnostics);
                eprintln!("{}", serde_json::to_string_pretty(&report).unwrap());
                return ExitCode::FAILURE;
            }
        }
    } else {
        format!(
            "{}\n",
            serde_json::to_string_pretty(&converted.value).unwrap()
        )
    };
    if !report.diagnostics.is_empty() {
        eprintln!("{}", serde_json::to_string_pretty(&report).unwrap());
    }
    print!("{text}");
    ExitCode::SUCCESS
}
