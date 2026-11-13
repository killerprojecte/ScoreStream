use std::{env, fs, process::ExitCode};
fn main() -> ExitCode {
    let args: Vec<_> = env::args().collect();
    if args.len() != 3 || !["validate", "inspect", "canonical"].contains(&args[1].as_str()) {
        eprintln!("Usage: scorestream <validate|inspect|canonical> <score.sfs>");
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
