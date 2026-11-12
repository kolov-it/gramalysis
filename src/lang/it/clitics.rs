//! Taking clitic pronouns off the end of a verb form.

/// Monosyllabic imperatives that double the clitic's consonant:
/// da' + mi = dammi, di' + lo = dillo, fa' + lo = fallo.
const DOUBLING_HOSTS: [&str; 5] = ["da", "di", "fa", "sta", "va"];
const COMBINING: [&str; 6] = ["glie", "me", "te", "se", "ce", "ve"];
const AFTER_COMBINING: [&str; 5] = ["lo", "la", "li", "le", "ne"];
const SINGLE: [&str; 11] = ["gli", "mi", "ti", "si", "ci", "vi", "lo", "la", "li", "le", "ne"];

/// Splits trailing clitics off a verb form: `dammelo` → (`da`, [me, lo]),
/// `prendendogliela` → (`prendendo`, [glie, la]). Takes the longest chain.
pub fn split_enclitics(form: &str) -> Option<(String, Vec<String>)> {
    let mut chain: Option<Vec<&str>> = None;
    'pairs: for first in COMBINING {
        for second in AFTER_COMBINING {
            if form.ends_with(&format!("{first}{second}")) && form.len() > first.len() + second.len() + 1 {
                chain = Some(vec![first, second]);
                break 'pairs;
            }
        }
    }
    if chain.is_none() {
        chain = SINGLE
            .iter()
            .find(|s| form.ends_with(*s) && form.len() > s.len() + 1)
            .map(|s| vec![*s]);
    }
    let chain = chain?;
    let chain_len: usize = chain.iter().map(|c| c.len()).sum();
    let mut host = form[..form.len() - chain_len].to_owned();
    let first_char = chain[0].chars().next()?;
    if host.ends_with(first_char) && DOUBLING_HOSTS.contains(&&host[..host.len() - first_char.len_utf8()]) {
        host.pop();
    }
    Some((host, chain.into_iter().map(str::to_owned).collect()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn split(s: &str) -> (String, Vec<String>) {
        split_enclitics(s).unwrap()
    }

    #[test]
    fn splits_clitic_chains() {
        assert_eq!(split("dammelo"), ("da".to_owned(), vec!["me".to_owned(), "lo".to_owned()]));
        assert_eq!(split("prendendogliela"), ("prendendo".to_owned(), vec!["glie".to_owned(), "la".to_owned()]));
        assert_eq!(split("dillo"), ("di".to_owned(), vec!["lo".to_owned()]));
        assert_eq!(split("dargli"), ("dar".to_owned(), vec!["gli".to_owned()]));
        assert_eq!(split("andarsene"), ("andar".to_owned(), vec!["se".to_owned(), "ne".to_owned()]));
        assert_eq!(split("ricordati"), ("ricorda".to_owned(), vec!["ti".to_owned()]));
    }
}
