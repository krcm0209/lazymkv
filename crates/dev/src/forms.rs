//! The engine forms of plan, decide and retune (design part 9.5; Ruling S87), read one way by lazymkvctl and by the
//! engine's `command` relay (Ruling W24).

use crate::Args;

/// What `--job`, `--jobs` and `--title` took out of a command's words, and the words left, unchanged and in order
/// (Rulings S87, T28-g). Def: T27 (in ctl), moved by T3-22. Use: T27, T3-22.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Forms {
    /// `--job`'s value.
    pub job: Option<String>,
    /// Whether `--jobs` was given.
    pub jobs: bool,
    /// `--title`'s value.
    pub title: Option<String>,
    /// The words left, for dev's parser.
    pub words: Vec<String>,
}

/// Takes `--job`, `--jobs` and `--title` out of `rest`, the words after `name`, through
/// `Args::new(rest, &["--job", "--jobs", "--title"])`, `flag`, `switch`, `flag` and `rest()`, and checks which of them
/// `name` allows, with ctl's texts unchanged (Ruling S87). Err is the argument error's text. Def: T27 (in ctl), moved
/// by T3-22. Use: T27, T3-22.
pub fn read_forms(name: &str, rest: &[String]) -> Result<Forms, String> {
    let mut args = Args::new(rest, &["--job", "--jobs", "--title"]);
    let job = args.flag("--job")?;
    let jobs = args.switch("--jobs");
    let title = args.flag("--title")?;
    let words = args.rest()?;
    if name == "retune" {
        if job.is_some() {
            return Err("argument --job: not allowed with retune; use --jobs".to_string());
        }
    } else if jobs {
        return Err("argument --jobs: only with retune".to_string());
    }
    if title.is_some() && job.is_none() {
        return Err("argument --title: only with --job".to_string());
    }
    if job.is_some() {
        match (name, &title) {
            ("decide", None) => return Err("argument --title: required with decide --job".to_string()),
            ("decide", Some(_)) => {}
            (_, Some(_)) => return Err(format!("argument --title: not allowed with {name}")),
            (_, None) => {}
        }
    }
    Ok(Forms { job, jobs, title, words })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(text: &[&str]) -> Vec<String> {
        text.iter().map(|word| word.to_string()).collect()
    }

    #[test]
    fn read_forms_keeps_ctls_errors_and_returns_the_title() {
        let forms = |job: Option<&str>, jobs: bool, title: Option<&str>, rest: &[&str]| Forms {
            job: job.map(str::to_string), jobs, title: title.map(str::to_string), words: words(rest),
        };
        assert_eq!(read_forms("decide", &words(&["--job", "a-1", "--t", "t00-00800", "--settings", "s.json"])),
                   Ok(forms(Some("a-1"), false, Some("t00-00800"), &["--settings", "s.json"])));
        assert_eq!(read_forms("retune", &words(&["--jobs", "--spec", "spec.md"])),
                   Ok(forms(None, true, None, &["--spec", "spec.md"])));
        assert_eq!(read_forms("plan", &words(&["slug", "--evidence", "e"])),
                   Ok(forms(None, false, None, &["slug", "--evidence", "e"])));
        for (name, given, message) in [
            ("plan", vec!["--title", "k"], "argument --title: only with --job"),
            ("decide", vec!["--job", "a-1"], "argument --title: required with decide --job"),
            ("plan", vec!["--job", "a-1", "--title", "k"], "argument --title: not allowed with plan"),
            ("retune", vec!["--job", "a-1"], "argument --job: not allowed with retune; use --jobs"),
            ("plan", vec!["--jobs"], "argument --jobs: only with retune"),
            ("plan", vec!["--jo", "a-1"], "ambiguous option: --jo could match --job, --jobs"),
            ("plan", vec!["--job"], "argument --job: expected one argument"),
        ] {
            assert_eq!(read_forms(name, &words(&given)), Err(message.to_string()), "{name} {given:?}");
        }
    }
}
