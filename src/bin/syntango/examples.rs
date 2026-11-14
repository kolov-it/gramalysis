//! `syntango examples`: example sentences for a verb from a corpus, as JSON
//! lines.

use std::fs::File;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use flate2::read::GzDecoder;
use serde::Deserialize;

use syntango::examples::{BoxError, Extractor, Options, Query, Sentence, VerbTables};
use syntango::lang::Language;
use syntango::morph::Lexicon;

/// Which corpus, in which format.
pub enum Corpus {
    /// JSON lines with `id`, `author`, `text` (data/<lang>/tatoeba-pool.jsonl.gz).
    Tatoeba(PathBuf),
    /// Leipzig `*-sentences.txt`: `id<TAB>sentence`.
    Leipzig(PathBuf),
    /// One sentence a line; the id is the line number. Lines starting with #
    /// are skipped.
    Lines(PathBuf),
}

pub struct Request {
    pub query: Query,
    pub corpus: Corpus,
    pub verbs: PathBuf,
    pub options: Options,
}

#[derive(Deserialize)]
struct TatoebaLine {
    id: u64,
    author: String,
    text: String,
}

pub async fn run(
    request: Request,
    lexicon: &dyn Lexicon,
    language: &dyn Language,
) -> Result<(), Box<dyn std::error::Error>> {
    let tables: VerbTables = serde_json::from_reader(BufReader::new(
        File::open(&request.verbs).map_err(|e| format!("{}: {e}", request.verbs.display()))?,
    ))
    .map_err(|e| format!("{}: {e}", request.verbs.display()))?;
    let extractor = Extractor::new(lexicon, language, &tables, &request.query)?;
    let started = Instant::now();
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let emit = |e: &syntango::examples::Example| -> Result<(), BoxError> {
        writeln!(out, "{}", serde_json::to_string(e)?)?;
        Ok(())
    };
    let stats = extractor
        .run(sentences(&request.corpus)?, &request.options, emit)
        .await?;
    let seconds = started.elapsed().as_secs_f64();
    eprintln!(
        "{} sentences, {} candidates ({} with an unknown word, {} without a parse), {} decided, {} ambiguous, {} printed; {:.1}s, {:.0} sentences/s, {:.1} candidates/s",
        stats.sentences,
        stats.candidates,
        stats.unknown_word,
        stats.no_parse,
        stats.decided,
        stats.ambiguous,
        stats.emitted,
        seconds,
        stats.sentences as f64 / seconds,
        stats.candidates as f64 / seconds,
    );
    Ok(())
}

fn open(path: &Path) -> Result<Box<dyn BufRead>, Box<dyn std::error::Error>> {
    let file = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let reader: Box<dyn Read> = if path.extension().is_some_and(|e| e == "gz") {
        Box::new(GzDecoder::new(file))
    } else {
        Box::new(file)
    };
    Ok(Box::new(BufReader::new(reader)))
}

type Sentences = Box<dyn Iterator<Item = Result<Sentence, BoxError>>>;

fn sentences(corpus: &Corpus) -> Result<Sentences, Box<dyn std::error::Error>> {
    Ok(match corpus {
        Corpus::Tatoeba(path) => {
            let source = path.display().to_string();
            Box::new(
                open(path)?
                    .lines()
                    .enumerate()
                    .filter(|(_, l)| l.as_ref().map_or(true, |l| !l.trim().is_empty()))
                    .map(move |(n, line)| {
                        let t: TatoebaLine = serde_json::from_str(&line?)
                            .map_err(|e| format!("{source}:{}: {e}", n + 1))?;
                        Ok(Sentence {
                            source: "tatoeba".to_owned(),
                            id: t.id,
                            author: Some(t.author),
                            text: t.text,
                        })
                    }),
            )
        }
        Corpus::Leipzig(path) => {
            let source = path.display().to_string();
            Box::new(open(path)?.lines().enumerate().map(move |(n, line)| {
                let line = line?;
                let (id, text) = line
                    .split_once('\t')
                    .ok_or_else(|| format!("{source}:{}: no tab", n + 1))?;
                let id = id
                    .parse()
                    .map_err(|e| format!("{source}:{}: id `{id}`: {e}", n + 1))?;
                Ok(Sentence {
                    source: "leipzig".to_owned(),
                    id,
                    author: None,
                    text: text.to_owned(),
                })
            }))
        }
        Corpus::Lines(path) => {
            let source = path.display().to_string();
            Box::new(
                open(path)?
                    .lines()
                    .enumerate()
                    .filter(|(_, l)| {
                        l.as_ref()
                            .map_or(true, |l| !l.trim().is_empty() && !l.starts_with('#'))
                    })
                    .map(move |(n, line)| {
                        Ok(Sentence {
                            source: source.clone(),
                            id: n as u64 + 1,
                            author: None,
                            text: line?.trim().to_owned(),
                        })
                    }),
            )
        }
    })
}
