//! The `gramalysis` command line: a language and a sentence in, the grammar
//! tree out.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand};

use gramalysis::lang::{self, Language};
use gramalysis::morph::{Lang, Lexicon};
use gramalysis::syntax::feats;
use gramalysis::wordholder::{FixtureLexicon, WordHolderLexicon, BATCH_ENV, KEY_ENV, URL_ENV};
use gramalysis::{analyze, SentenceAnalysis};

/// Parse sentences and print their grammar trees.
///
/// The lexicon is the Word Holder API. Its key is read only from the
/// environment (WORDHOLDER_API_KEY), so it never ends up in shell history;
/// the endpoint and batch size come from flags or the environment.
#[derive(Parser)]
#[command(version, args_conflicts_with_subcommands = true)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
    #[command(flatten)]
    parse: ParseArgs,
}

#[derive(Subcommand)]
enum Command {
    /// Save the API's answers for a sentence's words into a fixture file
    /// (grammar only; meanings are dropped).
    Record {
        file: PathBuf,
        lang: Lang,
        sentence: String,
        #[command(flatten)]
        backend: Backend,
    },
}

#[derive(Args)]
struct ParseArgs {
    /// Language code: it (es, fr, de, nl, en are planned).
    lang: Option<Lang>,
    /// The sentence. Quote it.
    sentence: Option<String>,
    /// Parse every line of FILE instead (lines starting with # are skipped).
    #[arg(long, conflicts_with = "sentence")]
    file: Option<PathBuf>,
    /// After each tree, the rule behind every node and why it applies.
    #[arg(long)]
    explain: bool,
    /// After the trees, every word's reading(s) across the parses.
    #[arg(long)]
    readings: bool,
    /// The full analysis as JSON instead of trees.
    #[arg(long)]
    json: bool,
    /// Answer from recorded API answers instead of calling the API.
    #[arg(long, value_name = "FILE")]
    fixture: Option<PathBuf>,
    #[command(flatten)]
    backend: Backend,
}

/// Where the lexicon lives. Each flag overrides its environment variable.
#[derive(Args)]
struct Backend {
    /// API base URL [env: WORDHOLDER_API_URL], e.g. https://word-holder.p.rapidapi.com
    #[arg(long, value_name = "URL")]
    api_url: Option<String>,
    /// Words per request; the RapidAPI free plan allows 10 [env: WORDHOLDER_API_BATCH]
    #[arg(long, value_name = "N")]
    batch: Option<usize>,
}

impl Backend {
    fn lexicon(&self) -> Result<WordHolderLexicon, String> {
        let url = self.api_url.clone().or_else(|| std::env::var(URL_ENV).ok());
        let batch = match self.batch {
            Some(n) => Some(n),
            None => match std::env::var(BATCH_ENV) {
                Ok(v) => Some(v.parse().map_err(|_| format!("{BATCH_ENV} is not a number: {v}"))?),
                Err(_) => None,
            },
        };
        let key = std::env::var(KEY_ENV).ok();
        let mut missing = Vec::new();
        if url.is_none() {
            missing.push(format!("the API URL (--api-url or {URL_ENV})"));
        }
        if batch.is_none() {
            missing.push(format!("the batch size (--batch or {BATCH_ENV})"));
        }
        if key.is_none() {
            missing.push(format!("the API key ({KEY_ENV})"));
        }
        match (url, batch, key) {
            (Some(url), Some(batch), Some(key)) => WordHolderLexicon::new(&url, key, batch).map_err(|e| e.to_string()),
            _ => Err(format!(
                "the lexicon backend is not configured; missing {}.\nSee README: \"With your own key\", or use --fixture FILE to run offline.",
                missing.join(", ")
            )),
        }
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    match run(Cli::parse()).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

async fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(Command::Record { file, lang, sentence, backend }) = cli.command {
        return record(&file, lang, &sentence, &backend.lexicon()?).await;
    }
    let args = cli.parse;
    let lang = args.lang.ok_or("give a language and a sentence: gramalysis it \"La ragazza ama il cane.\"")?;
    let sentences: Vec<String> = match (&args.file, &args.sentence) {
        (Some(path), _) => std::fs::read_to_string(path)?
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(str::to_owned)
            .collect(),
        (None, Some(s)) => vec![s.clone()],
        (None, None) => return Err("give a sentence, or --file FILE".into()),
    };
    let language = lang::load(lang)?;
    let lexicon: Box<dyn Lexicon> = match &args.fixture {
        Some(path) => Box::new(FixtureLexicon::load(path)?),
        None => Box::new(args.backend.lexicon()?),
    };
    for (i, text) in sentences.iter().enumerate() {
        let a = analyze(lexicon.as_ref(), language.as_ref(), text).await?;
        if args.json {
            println!("{}", serde_json::to_string_pretty(&a)?);
            continue;
        }
        if i > 0 {
            println!();
        }
        print_analysis(text, &a, args.explain, args.readings);
    }
    Ok(())
}

fn print_analysis(text: &str, a: &SentenceAnalysis, explain: bool, readings: bool) {
    println!("{text}");
    if !a.unknown.is_empty() {
        println!("  unknown words: {}", a.unknown.join(", "));
    }
    match a.parses.len() {
        0 => println!("  no parse: the grammar does not accept this sentence"),
        1 => {}
        n => println!("  {n} parses"),
    }
    for (i, p) in a.parses.iter().enumerate() {
        if a.parses.len() > 1 {
            println!("\nparse {} of {}", i + 1, a.parses.len());
        }
        print!("{}", p.render());
        if explain {
            let mut seen = std::collections::HashSet::new();
            for (rule, why) in p.rules() {
                if seen.insert(rule) {
                    println!("  · {rule}: {why}");
                }
            }
        }
    }
    if readings {
        println!("\nreadings (✓ = every parse agrees):");
        for c in &a.chosen {
            let rs: Vec<String> = c
                .readings
                .iter()
                .map(|r| r.iter().map(|m| format!("{} {} {}", m.lemma, m.cat, feats::display(&m.feats))).collect::<Vec<_>>().join(" + "))
                .collect();
            let mark = if c.agreed { "✓" } else if rs.is_empty() { "?" } else { "~" };
            println!("  {mark} {}: {}", c.token, rs.join("  |  "));
        }
    }
}

async fn record(file: &PathBuf, lang: Lang, text: &str, lexicon: &WordHolderLexicon) -> Result<(), Box<dyn std::error::Error>> {
    let language: Box<dyn Language> = lang::load(lang)?;
    let forms = language.lexicon_forms(&language.tokenize(text));
    let refs: Vec<&str> = forms.iter().map(String::as_str).collect();
    let mut data: serde_json::Value = match std::fs::read_to_string(file) {
        Ok(s) => serde_json::from_str(&s)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => serde_json::json!({}),
        Err(e) => return Err(e.into()),
    };
    for (form, mut lemmas) in lexicon.raw(lang, &refs).await? {
        // Keep the grammar, drop the meanings: glosses are Wiktionary text
        // (CC BY-SA) and the parser does not use them.
        for entry in lemmas.as_array_mut().into_iter().flatten() {
            if let Some(obj) = entry.as_object_mut() {
                obj.remove("glosses");
            }
        }
        data[lang.code()][form.as_str()] = lemmas;
    }
    std::fs::write(file, serde_json::to_string_pretty(&data)? + "\n")?;
    println!("recorded {} forms into {}", refs.len(), file.display());
    Ok(())
}
