use scorestream_formats::{ScoreAdapter, musicxml::MusicXml, sfs::SfsAdapter};
use std::{env, fs, io::BufReader, process::ExitCode};
fn main() -> ExitCode {
    let args: Vec<_> = env::args().collect();
    if args
        .get(1)
        .is_some_and(|v| v == "convert" || v == "inspect-format" || v == "scan-musicxml")
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
        "Usage:\n  scorestream <validate|inspect|canonical> <score.sfs>\n  scorestream inspect-format <sfs|musicxml> <input>\n  scorestream convert <sfs|musicxml> <sfs|musicxml> <input>\n  scorestream scan-musicxml <input.musicxml>\nConverted text goes to stdout; structured diagnostics go to stderr. Unsupported mappings produce no output."
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
    if args[1] == "scan-musicxml" {
        if args.len() != 3 {
            usage();
            return ExitCode::from(2);
        }
        let file = match fs::File::open(&args[2]) {
            Ok(file) => file,
            Err(error) => {
                eprintln!("{error}");
                return ExitCode::from(2);
            }
        };
        return match scorestream_formats::musicxml::scan(BufReader::new(file)) {
            Ok(summary) => {
                println!("{}", serde_json::to_string_pretty(&summary).unwrap());
                ExitCode::SUCCESS
            }
            Err(report) => {
                eprintln!("{}", serde_json::to_string_pretty(&report).unwrap());
                ExitCode::FAILURE
            }
        };
    }
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
    let result = if args[2] == "musicxml" {
        let file = match fs::File::open(path) {
            Ok(file) => file,
            Err(error) => {
                eprintln!("{error}");
                return ExitCode::from(2);
            }
        };
        scorestream_formats::musicxml::read_reader(BufReader::new(file))
    } else {
        // SFS has its own text parser; an XML byte budget must not reject valid SFS input.
        let source = match fs::read_to_string(path) {
            Ok(source) => source,
            Err(error) => {
                eprintln!("{error}");
                return ExitCode::from(2);
            }
        };
        input.read(&source)
    };
    let converted = match result {
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
                report.append(result.report);
                result.value
            }
            Err(result) => {
                report.append(result);
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
