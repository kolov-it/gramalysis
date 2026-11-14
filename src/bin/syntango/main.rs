//! The `syntango` command line: a language and a sentence in, the grammar
//! tree out.

mod eval;
mod examples;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{ArgGroup, Args, Parser, Subcommand};

use syntango::examples::tenses::tenses;
use syntango::examples::{Options, Query};
use syntango::lang::{self, Language};
use syntango::morph::{Lang, Lexicon};
use syntango::syntax::feats;
use syntango::patch::PatchedLexicon;
use syntango::wordholder::{self, FixtureLexicon, WordHolderLexicon, BATCH_ENV, KEY_ENV, URL_ENV};
use syntango::{analyze, SentenceAnalysis};

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
    /// (grammar only; meanings are dropped). Words already in the file are
    /// not asked again, and the file is saved after every request, so a run
    /// stopped by a rate limit continues where it stopped.
    Record {
        file: PathBuf,
        lang: Lang,
        /// The sentence. Quote it.
        sentence: Option<String>,
        /// Record every word of a sentence set (JSON lines with a `text` field).
        #[arg(long, conflicts_with = "sentence")]
        set: Option<PathBuf>,
        #[command(flatten)]
        backend: Backend,
    },
    /// Run a sentence set and measure how its ambiguous words were read.
    Eval {
        lang: Lang,
        /// The set, e.g. data/it/homograph-set.jsonl.
        set: PathBuf,
        /// Correct readings, e.g. data/it/homograph-labels.tsv.
        #[arg(long)]
        labels: Option<PathBuf>,
        /// Write one JSON line per sentence here.
        #[arg(long, value_name = "FILE")]
        out: Option<PathBuf>,
        /// Answer from recorded API answers instead of calling the API.
        #[arg(long, value_name = "FILE")]
        fixture: Option<PathBuf>,
        /// Use the lexicon's answers as they are, without data/<lang>/lexicon-patches.
        #[arg(long)]
        no_patches: bool,
        #[command(flatten)]
        backend: Backend,
    },
    /// List a language's tenses: the identifier --tense takes, the native and
    /// English names, simple or compound, and for a compound tense the
    /// auxiliary's tense it is built from.
    Tenses {
        lang: Lang,
        /// As a JSON array.
        #[arg(long)]
        json: bool,
    },
    /// Find example sentences for a verb in a corpus and print them as JSON
    /// lines: the sentence, where the verb form is in it, which cell it is,
    /// and whether every parse reads it so (`decided`) or not (`ambiguous`).
    /// Counts and throughput go to stderr.
    #[command(group(ArgGroup::new("corpus").required(true).args(["tatoeba", "leipzig", "file"])))]
    Examples {
        lang: Lang,
        /// The verb as its table names it, e.g. essere, farsi.
        infinitive: String,
        /// Only this tense or cell, e.g. indicative.past_historic,
        /// indicative.past_historic.3s, non_finite.gerund.
        #[arg(long)]
        tense: Option<String>,
        /// Tatoeba sentences: JSON lines with id, author, text (.gz is read
        /// as gzip), e.g. data/it/tatoeba-pool.jsonl.gz.
        #[arg(long, value_name = "FILE")]
        tatoeba: Option<PathBuf>,
        /// Leipzig corpus sentences: id<TAB>sentence (.gz is read as gzip).
        #[arg(long, value_name = "FILE")]
        leipzig: Option<PathBuf>,
        /// One sentence a line; the id is the line number.
        #[arg(long, value_name = "FILE")]
        file: Option<PathBuf>,
        /// The verb tables [default: data/<lang>/verbs.json].
        #[arg(long, value_name = "FILE")]
        verbs: Option<PathBuf>,
        /// Stop after N examples.
        #[arg(long, value_name = "N")]
        limit: Option<usize>,
        /// Print ambiguous examples too.
        #[arg(long)]
        include_ambiguous: bool,
        /// Answer from recorded API answers instead of calling the API.
        #[arg(long, value_name = "FILE")]
        fixture: Option<PathBuf>,
        /// Use the lexicon's answers as they are, without data/<lang>/lexicon-patches.
        #[arg(long)]
        no_patches: bool,
        #[command(flatten)]
        backend: Backend,
    },
}

#[derive(Args)]
struct ParseArgs {
    /// Language code: it, es or fr (de, nl, en are planned).
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
    /// After the trees, every word's grammatical role (subject, verb,
    /// direct-object…) across the parses.
    #[arg(long)]
    roles: bool,
    /// The full analysis as JSON instead of trees.
    #[arg(long)]
    json: bool,
    /// Answer from recorded API answers instead of calling the API.
    #[arg(long, value_name = "FILE")]
    fixture: Option<PathBuf>,
    /// Use the lexicon's answers as they are, without data/<lang>/lexicon-patches.
    #[arg(long)]
    no_patches: bool,
    #[command(flatten)]
    backend: Backend,
}

/// The lexicon to use: recorded answers or the API, with or without patches.
fn lexicon(fixture: Option<&Path>, backend: &Backend, no_patches: bool) -> Result<Box<dyn Lexicon>, Box<dyn std::error::Error>> {
    let base: Box<dyn Lexicon> = match fixture {
        Some(path) => Box::new(FixtureLexicon::load(path)?),
        None => Box::new(backend.lexicon()?),
    };
    if no_patches {
        return Ok(base);
    }
    Ok(Box::new(PatchedLexicon::new(base, wordholder::patches()?)))
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
    match cli.command {
        Some(Command::Record { file, lang, sentence, set, backend }) => {
            let texts = match (sentence, set) {
                (Some(s), None) => vec![s],
                (None, Some(path)) => eval::load_items(&path)?.into_iter().map(|i| i.text).collect(),
                _ => return Err("give a sentence or --set FILE".into()),
            };
            return record(&file, lang, &texts, &backend.lexicon()?).await;
        }
        Some(Command::Eval { lang, set, labels, out, fixture, no_patches, backend }) => {
            let language = lang::load(lang)?;
            let lexicon = lexicon(fixture.as_deref(), &backend, no_patches)?;
            let labels = match &labels {
                Some(path) => eval::load_labels(path)?,
                None => Default::default(),
            };
            let items = eval::load_items(&set)?;
            return eval::run(&items, &labels, lexicon.as_ref(), language.as_ref(), out.as_deref()).await;
        }
        Some(Command::Tenses { lang, json }) => {
            let all = tenses(lang);
            if all.is_empty() {
                return Err(format!("{} is not supported yet; supported: it, es, fr", lang.code()).into());
            }
            if json {
                println!("{}", serde_json::to_string_pretty(all)?);
                return Ok(());
            }
            for t in all {
                let kind = match t.auxiliary_tense {
                    Some(aux) => format!("compound: auxiliary in {aux}"),
                    None => "simple".to_owned(),
                };
                println!("{:<32} {:<42} {:<26} {kind}", t.id, t.native, t.english);
            }
            return Ok(());
        }
        Some(Command::Examples {
            lang,
            infinitive,
            tense,
            tatoeba,
            leipzig,
            file,
            verbs,
            limit,
            include_ambiguous,
            fixture,
            no_patches,
            backend,
        }) => {
            let corpus = match (tatoeba, leipzig, file) {
                (Some(p), None, None) => examples::Corpus::Tatoeba(p),
                (None, Some(p), None) => examples::Corpus::Leipzig(p),
                (None, None, Some(p)) => examples::Corpus::Lines(p),
                _ => return Err("give one of --tatoeba, --leipzig, --file".into()),
            };
            let language = lang::load(lang)?;
            let lexicon = lexicon(fixture.as_deref(), &backend, no_patches)?;
            let request = examples::Request {
                query: Query { infinitive, tense },
                corpus,
                verbs: verbs.unwrap_or_else(|| PathBuf::from(format!("data/{}/verbs.json", lang.code()))),
                options: Options { limit, include_ambiguous },
            };
            return examples::run(request, lexicon.as_ref(), language.as_ref()).await;
        }
        None => {}
    }
    let args = cli.parse;
    let lang = args.lang.ok_or("give a language and a sentence: syntango it \"La ragazza ama il cane.\"")?;
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
    let lexicon = lexicon(args.fixture.as_deref(), &args.backend, args.no_patches)?;
    for (i, text) in sentences.iter().enumerate() {
        let a = analyze(lexicon.as_ref(), language.as_ref(), text).await?;
        if args.json {
            println!("{}", serde_json::to_string_pretty(&a)?);
            continue;
        }
        if i > 0 {
            println!();
        }
        print_analysis(text, &a, args.explain, args.readings, args.roles);
    }
    Ok(())
}

fn print_analysis(text: &str, a: &SentenceAnalysis, explain: bool, readings: bool, roles: bool) {
    println!("{text}");
    if !a.unknown.is_empty() {
        println!("  unknown words: {}", a.unknown.join(", "));
    }
    match a.parse_count {
        0 => println!("  no parse: the grammar does not accept this sentence"),
        1 => {}
        n if n as usize > a.parses.len() => println!("  {n} parses (showing {})", a.parses.len()),
        n => println!("  {n} parses"),
    }
    for (i, p) in a.parses.iter().enumerate() {
        if a.parses.len() > 1 {
            println!("\nparse {} of {}", i + 1, a.parse_count);
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
                .zip(&c.uses)
                .map(|(r, uses)| {
                    let text = r.iter().map(|m| format!("{} {} {}", m.lemma, m.cat, feats::display(&m.feats))).collect::<Vec<_>>().join(" + ");
                    let adjective =
                        |u: &String| u == "ap.participle" || u == "ap.present-participle";
                    if uses.iter().any(adjective) {
                        let how: Vec<&str> = uses
                            .iter()
                            .map(|u| {
                                if adjective(u) {
                                    "adjective"
                                } else {
                                    u.as_str()
                                }
                            })
                            .collect();
                        format!("{text} [used as: {}]", how.join(", "))
                    } else {
                        text
                    }
                })
                .collect();
            let mark = if c.agreed { "✓" } else if rs.is_empty() { "?" } else { "~" };
            println!("  {mark} {}: {}", c.token, rs.join("  |  "));
        }
    }
    if roles {
        print_roles(a);
    }
}

fn print_roles(a: &SentenceAnalysis) {
    println!("\nroles (✓ = every parse agrees, - = no role):");
    for t in &a.roles {
        let rs: Vec<String> = t.roles.iter().map(|r| r.map_or("-".to_owned(), |r| r.to_string())).collect();
        let mark = if t.agreed { "✓" } else if rs.is_empty() { "?" } else { "~" };
        let rs = if rs.is_empty() { "no parse".to_owned() } else { rs.join("  |  ") };
        println!("  {mark} {}: {rs}", t.token);
    }
}

async fn record(file: &Path, lang: Lang, texts: &[String], lexicon: &WordHolderLexicon) -> Result<(), Box<dyn std::error::Error>> {
    let language: Box<dyn Language> = lang::load(lang)?;
    let mut data: serde_json::Value = match std::fs::read_to_string(file) {
        Ok(s) => serde_json::from_str(&s)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => serde_json::json!({}),
        Err(e) => return Err(e.into()),
    };
    let mut forms: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for text in texts {
        for form in language.lexicon_forms(&language.tokenize(text)) {
            if data[lang.code()].get(&form).is_none() && seen.insert(form.clone()) {
                forms.push(form);
            }
        }
    }
    println!("{} forms to record", forms.len());
    for (n, chunk) in forms.chunks(lexicon.batch_size()).enumerate() {
        let refs: Vec<&str> = chunk.iter().map(String::as_str).collect();
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
        if (n + 1) % 10 == 0 {
            println!("  {} of {}", ((n + 1) * lexicon.batch_size()).min(forms.len()), forms.len());
        }
    }
    println!("recorded into {}", file.display());
    Ok(())
}
