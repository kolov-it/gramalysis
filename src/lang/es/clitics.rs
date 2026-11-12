//! Taking clitic pronouns off the end of a Spanish verb form. An infinitive,
//! a gerund or an affirmative imperative carries them attached, and the host
//! often gains a written accent to keep its stress (da + me + lo = dámelo,
//! diciendo + se + lo = diciéndoselo). The nosotros imperative loses its -s
//! before nos (vamos + nos = vámonos), the vosotros imperative its -d before
//! os (sentad + os = sentaos).

const FIRST: [&str; 5] = ["me", "te", "se", "nos", "os"];
const SECOND: [&str; 8] = ["lo", "la", "los", "las", "le", "les", "me", "nos"];
const SINGLE: [&str; 11] = ["me", "te", "se", "nos", "os", "lo", "la", "los", "las", "le", "les"];
/// One-syllable imperatives, which take clitics without an accent (dame,
/// hazlo, ponte, dime, vete, deme).
const SHORT_IMPERATIVES: [&str; 11] = ["da", "de", "di", "haz", "pon", "ten", "sal", "ven", "ve", "sé", "id"];

/// One way to read a form as host + clitics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Split {
    /// The host's spellings to look up: as written, and without the accent
    /// the clitics added (dá → da); a lexical accent (oír) stays in the
    /// first.
    pub hosts: Vec<String>,
    pub clitics: Vec<String>,
}

/// Every plausible host + clitics reading of `form` (lower case), longest
/// clitic chain first. The lexicon decides which host is a verb form.
pub fn split_enclitics(form: &str) -> Vec<Split> {
    let mut chains: Vec<Vec<&str>> = Vec::new();
    for first in FIRST {
        for second in SECOND {
            if first == second {
                continue;
            }
            if form.ends_with(&format!("{first}{second}")) {
                chains.push(vec![first, second]);
            }
        }
    }
    for single in SINGLE {
        if form.ends_with(single) {
            chains.push(vec![single]);
        }
    }
    let accented = form.chars().any(|c| "áéíóú".contains(c));
    let mut out = Vec::new();
    for chain in chains {
        let chain_len: usize = chain.iter().map(|c| c.len()).sum();
        let mut host = form[..form.len() - chain_len].to_owned();
        if host.chars().count() < 2 {
            continue;
        }
        // vámonos, sentémonos: the -s of -mos falls before nos.
        if chain[0] == "nos" && strip_accent(&host).ends_with("mo") {
            host.push('s');
        }
        // sentaos, decíos: the -d of the vosotros imperative falls before os.
        if chain[0] == "os" && host.ends_with(['a', 'e', 'i', 'í']) {
            host.push('d');
        }
        let plain = strip_accent(&host);
        // Hosts that can carry clitics: an infinitive, a gerund, a vosotros
        // imperative, a short imperative (da, haz, pon, di), or any form the
        // clitics forced an accent on (llámalo, hágalo, sentémonos).
        let plausible = plain.ends_with('r')
            || plain.ends_with("ndo")
            || plain.ends_with('d')
            || SHORT_IMPERATIVES.contains(&host.as_str())
            || accented;
        if !plausible {
            continue;
        }
        let mut hosts = vec![host.clone()];
        if plain != host {
            hosts.push(plain);
        }
        out.push(Split { hosts, clitics: chain.into_iter().map(str::to_owned).collect() });
    }
    out
}

/// The form without its written accents (ñ and ü stay).
pub fn strip_accent(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'á' => 'a',
            'é' => 'e',
            'í' => 'i',
            'ó' => 'o',
            'ú' => 'u',
            other => other,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn has(form: &str, host: &str, clitics: &[&str]) -> bool {
        split_enclitics(form).iter().any(|s| s.hosts.iter().any(|h| h == host) && s.clitics == clitics)
    }

    #[test]
    fn splits_clitic_chains_and_the_added_accent() {
        assert!(has("dámelo", "da", &["me", "lo"]));
        assert!(has("diciéndoselo", "diciendo", &["se", "lo"]));
        assert!(has("hazlo", "haz", &["lo"]));
        assert!(has("hacerlo", "hacer", &["lo"]));
        assert!(has("oírlo", "oír", &["lo"]));
        assert!(has("vámonos", "vamos", &["nos"]));
        assert!(has("sentémonos", "sentemos", &["nos"]));
        assert!(has("sentaos", "sentad", &["os"]));
        assert!(has("dígaselo", "diga", &["se", "lo"]));
    }

    #[test]
    fn leaves_ordinary_words_alone() {
        assert!(split_enclitics("hablo").is_empty());
        assert!(split_enclitics("me").is_empty());
    }
}
