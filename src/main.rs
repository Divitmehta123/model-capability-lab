use model_capability_lab::{
    Capability, eligible, evaluate, parse_tsv, render_json, render_markdown, render_text,
};
use std::{env, fs, process::ExitCode};
fn usage() -> &'static str {
    "model-capability-lab [--input FILE] [--format text|markdown|json] [--min-score N] [--require CAPABILITY]\nCapabilities: tool_calls, vision, json_schema, streaming, long_context"
}
fn run() -> Result<bool, String> {
    let mut input = "fixtures/default.tsv".to_string();
    let mut format = "text".to_string();
    let mut minimum = 0u16;
    let mut required = Vec::new();
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--input" => input = args.next().ok_or("--input requires a file")?,
            "--format" => format = args.next().ok_or("--format requires a value")?,
            "--min-score" => {
                minimum = args
                    .next()
                    .ok_or("--min-score requires a number")?
                    .parse()
                    .map_err(|_| "invalid minimum score")?
            }
            "--require" => {
                let value = args.next().ok_or("--require requires a capability")?;
                required.push(
                    Capability::parse(&value)
                        .ok_or_else(|| format!("unknown capability `{value}`"))?,
                );
            }
            "--help" | "-h" => {
                println!("{}", usage());
                return Ok(true);
            }
            other => return Err(format!("unknown argument `{other}`\n{}", usage())),
        }
    }
    let source = fs::read_to_string(&input).map_err(|e| format!("failed to read {input}: {e}"))?;
    let results = evaluate(&parse_tsv(&source)?);
    let output = match format.as_str() {
        "text" => render_text(&results),
        "markdown" => render_markdown(&results),
        "json" => render_json(&results),
        _ => return Err("format must be text, markdown, or json".into()),
    };
    println!("{output}");
    let matching = eligible(&results, minimum, &required);
    if minimum > 0 || !required.is_empty() {
        eprintln!(
            "gate: {} of {} models eligible",
            matching.len(),
            results.len()
        );
    }
    Ok((minimum == 0 && required.is_empty()) || !matching.is_empty())
}
fn main() -> ExitCode {
    match run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(2)
        }
    }
}
