// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 CapCraft contributors

//! How English words are said, where spelling does not tell.
//!
//! "Live" in "the match is broadcast live" is not "live" in "how you live",
//! and a voice that looks words up reads both alike. [`respell`] tells such
//! words by the ones about them, and where it is sure, spells them as they
//! sound ("lyve", "liv"), which every voice reads one way. Where it is not
//! sure it leaves the word to the voice. [`in_words`] writes figures out
//! for a voice that stumbles on them.

/// One stretch of the text: a word, or what is between two.
#[derive(Debug, Clone, Copy)]
struct Piece<'a> {
    text: &'a str,
    word: bool,
}

fn pieces(text: &str) -> Vec<Piece<'_>> {
    let mut out: Vec<Piece<'_>> = Vec::new();
    let mut start = 0;
    let mut was: Option<bool> = None;
    for (at, c) in text.char_indices() {
        let word = c.is_alphanumeric() || c == '\'' || c == '\u{2019}';
        if was.is_some_and(|w| w != word) {
            out.push(Piece {
                text: &text[start..at],
                word: !word,
            });
            start = at;
        }
        was = Some(word);
    }
    if let Some(word) = was {
        out.push(Piece {
            text: &text[start..],
            word,
        });
    }
    out
}

/// The words about one, lowercase; empty past the sentence's ends.
struct About {
    /// Two before.
    second: String,
    before: String,
    after: String,
    /// Every word of the sentence.
    all: Vec<String>,
}

fn is(word: &str, list: &[&str]) -> bool {
    list.contains(&word)
}

const BE: &[&str] = &[
    "is", "was", "are", "were", "be", "being", "been", "am", "it's", "we're", "i'm", "you're",
    "they're", "that's", "he's", "she's", "isn't", "wasn't", "aren't", "weren't", "here's",
];
/// What a verb in its plain form follows.
const BEFORE_VERB: &[&str] = &[
    "to", "can", "can't", "cannot", "could", "couldn't", "will", "won't", "would", "wouldn't",
    "should", "shouldn't", "must", "may", "might", "don't", "doesn't", "didn't", "do", "does",
    "did", "not", "never", "always", "i'll", "we'll", "you'll", "they'll", "he'll", "she'll",
    "i'd", "we'd", "you'd", "they'd", "let's", "please", "rather",
];
const SUBJECT: &[&str] = &["i", "you", "we", "they", "who", "people", "those", "both", "all"];

/// How this "live" is said: `Some(true)` as in "alive", `Some(false)` as in
/// "give".
fn live(about: &About) -> Option<bool> {
    const SHOWN: &[&str] = &[
        "show", "shows", "stream", "streams", "streaming", "streamed", "music", "broadcast",
        "broadcasts", "event", "events", "concert", "concerts", "wire", "wires", "video",
        "videos", "score", "scores", "coverage", "performance", "performances", "audience",
        "update", "updates", "feed", "feeds", "tv", "television", "telecast", "match",
        "matches", "chat", "session", "sessions", "action", "demo", "band", "bands",
        "ammunition", "ammo", "rounds", "bait", "animals", "recording", "album", "tour",
        "report", "reporting", "commentary", "game", "games", "sports", "news", "radio",
        "photo", "photos", "wallpaper", "interview", "webinar", "class", "classes", "cam",
        "camera", "preview", "captions", "data", "site", "link", "production", "theatre",
        "theater", "comedy",
    ];
    const DONE: &[&str] = &[
        "go", "goes", "going", "went", "gone", "broadcast", "broadcasts", "broadcasted",
        "broadcasting", "stream", "streamed", "streams", "streaming", "air", "aired", "airing",
        "airs", "watch", "watching", "watched", "perform", "performed", "performing",
        "performs", "play", "played", "playing", "recorded", "shown", "telecast", "telecasted",
        "coming", "reporting", "sing", "sang", "singing", "sung", "speaking", "appear",
        "appeared", "appearing", "shot", "filmed", "covered", "relayed", "presented",
        "presenting", "available", "experience", "goes", "join", "almost", "back", "seen",
        "heard", "televised",
    ];
    const IT: &[&str] = &["it", "this", "that", "them", "us", "me", "him", "her"];
    const AFTER_IT: &[&str] = &["", "on", "at", "from", "in", "now", "here", "tonight", "today"];
    let (second, before, after) = (
        about.second.as_str(),
        about.before.as_str(),
        about.after.as_str(),
    );
    if is(after, SHOWN) {
        return Some(true);
    }
    if is(before, &["a", "the", "this", "first", "more", "some", "our", "your", "his", "her"]) {
        // "your live" something; "her live" is left when it is "let her live"
        return (before != "her" || second != "let").then_some(true);
    }
    if before.is_empty() && is(after, &["from", "at", "on", "now", "tonight", "today"]) {
        return Some(true);
    }
    // "is now live", but "I now live"
    if is(before, &["now", "currently", "officially", "finally", "still", "also", "not"]) && is(second, BE) {
        return Some(true);
    }
    if is(before, BE) || is(before, DONE) {
        return Some(true);
    }
    if is(before, IT) && is(after, AFTER_IT) {
        // "is it live", "watch it live", but "let it live", "makes me live"
        return if is(second, BE) || is(second, DONE) || is(second, &["see", "saw", "catch", "hear", "seeing"]) {
            Some(true)
        } else {
            is(second, &["let", "helps", "help", "make", "makes"]).then_some(false)
        };
    }
    if is(before, SUBJECT) {
        // "are you live?", but "how you live", "where do you live", "you live once"
        let asked = is(second, &["are", "were", "aren't", "weren't", "am"]);
        return Some(asked && is(after, &["", "now", "on", "yet", "today", "tonight"]));
    }
    if is(before, BEFORE_VERB) {
        return Some(false);
    }
    if before.is_empty() && is(after, &["your", "a", "the", "life", "every", "each", "well", "long", "with", "for", "and", "in", "by", "like", "as", "laugh"]) {
        return Some(false);
    }
    None
}

/// Whether this "read" is the past one, said "red".
fn read_past(about: &About) -> Option<bool> {
    const HAD: &[&str] = &[
        "have", "has", "had", "having", "i've", "we've", "you've", "they've", "hasn't",
        "haven't", "hadn't", "been", "was", "were", "be", "being", "is", "are", "already",
        "he", "she", "it", "just", "ever",
    ];
    const THEN: &[&str] = &["yesterday", "ago", "last", "once", "earlier"];
    if is(&about.before, BEFORE_VERB) {
        return Some(false);
    }
    if is(&about.before, &["is", "are", "was", "were", "be", "being"]) && about.after == "only" {
        return None;
    }
    if is(&about.before, HAD) {
        return Some(true);
    }
    if is(&about.before, SUBJECT) && about.all.iter().any(|w| is(w, THEN)) {
        return Some(true);
    }
    None
}

/// The word as it sounds here, or `None` to leave it as it is.
fn sounded(word: &str, about: &About) -> Option<&'static str> {
    match word {
        "live" => live(about).map(|alive| if alive { "lyve" } else { "liv" }),
        "read" => read_past(about).and_then(|past| past.then_some("red")),
        "lead" => (is(
            &about.after,
            &["pipe", "pipes", "paint", "poisoning", "acid", "bullet", "bullets", "weight", "weights", "pencil", "free"],
        ) || is(&about.before, &["molten", "liquid", "contains", "toxic"]))
        .then_some("led"),
        "wind" => ((is(&about.before, BEFORE_VERB) || about.before.is_empty())
            && is(&about.after, &["the", "up", "down", "it", "your", "this", "back"])
            && about.before != "not")
            .then_some("wynd"),
        "tear" => ((is(&about.before, BEFORE_VERB) || about.before.is_empty())
            && is(&about.after, &["the", "it", "up", "down", "apart", "off", "this", "your", "them", "out", "open", "along", "here"]))
        .then_some("tare"),
        _ => None,
    }
}

/// `text` with the words whose sound its spelling does not tell spelt as
/// they are said here. Anything but English is handed back as it is.
pub fn respell(text: &str) -> String {
    let parts = pieces(text);
    let ends = |piece: &Piece<'_>| !piece.word && piece.text.contains(['.', '!', '?', '\n', ';', ':', '\u{2026}']);
    let lower = |at: usize| parts[at].text.to_lowercase().replace('\u{2019}', "'");
    let mut out = String::with_capacity(text.len() + 8);
    for (at, piece) in parts.iter().enumerate() {
        let word = piece.word.then(|| lower(at)).unwrap_or_default();
        if !is(&word, &["live", "read", "lead", "wind", "tear"]) {
            out.push_str(piece.text);
            continue;
        }
        let mut earlier: Vec<String> = (0..at)
            .rev()
            .take_while(|i| !ends(&parts[*i]))
            .filter(|i| parts[*i].word)
            .map(lower)
            .collect();
        let later: Vec<String> = (at + 1..parts.len())
            .take_while(|i| !ends(&parts[*i]))
            .filter(|i| parts[*i].word)
            .map(lower)
            .collect();
        // a comma stands between a word and the one after it
        let cut = parts.get(at + 1).is_some_and(|gap| gap.text.contains(','));
        let about = About {
            second: earlier.get(1).cloned().unwrap_or_default(),
            before: earlier.first().cloned().unwrap_or_default(),
            after: if cut {
                String::new()
            } else {
                later.first().cloned().unwrap_or_default()
            },
            all: {
                earlier.extend(later);
                earlier
            },
        };
        match sounded(&word, &about) {
            Some(said) if piece.text.chars().next().is_some_and(char::is_uppercase) => {
                let mut chars = said.chars();
                if let Some(first) = chars.next() {
                    out.extend(first.to_uppercase());
                    out.push_str(chars.as_str());
                }
            }
            Some(said) => out.push_str(said),
            None => out.push_str(piece.text),
        }
    }
    out
}

const ONES: [&str; 20] = [
    "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
    "eleven", "twelve", "thirteen", "fourteen", "fifteen", "sixteen", "seventeen", "eighteen",
    "nineteen",
];
const TENS: [&str; 10] = [
    "", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
];

/// A whole number as it is said.
fn number(n: u64) -> String {
    match n {
        0..=19 => ONES[n as usize].to_owned(),
        20..=99 => {
            let tens = TENS[(n / 10) as usize];
            match n % 10 {
                0 => tens.to_owned(),
                one => format!("{tens} {}", ONES[one as usize]),
            }
        }
        100..=999 => with_rest(format!("{} hundred", ONES[(n / 100) as usize]), n % 100, " and "),
        1_000..=999_999 => with_rest(format!("{} thousand", number(n / 1_000)), n % 1_000, " "),
        1_000_000..=999_999_999 => {
            with_rest(format!("{} million", number(n / 1_000_000)), n % 1_000_000, " ")
        }
        _ => with_rest(
            format!("{} billion", number(n / 1_000_000_000)),
            n % 1_000_000_000,
            " ",
        ),
    }
}

fn with_rest(head: String, rest: u64, joint: &str) -> String {
    if rest == 0 {
        head
    } else if rest < 100 {
        format!("{head} and {}", number(rest))
    } else {
        format!("{head}{joint}{}", number(rest))
    }
}

/// A year as it is said: 1998 is "nineteen ninety eight".
fn year(n: u64) -> String {
    let (high, low) = (n / 100, n % 100);
    if (2000..2010).contains(&n) || low == 0 && n % 1000 == 0 {
        number(n)
    } else if low == 0 {
        format!("{} hundred", number(high))
    } else if low < 10 {
        format!("{} oh {}", number(high), number(low))
    } else {
        format!("{} {}", number(high), number(low))
    }
}

fn digits(text: &str) -> String {
    text.chars()
        .filter_map(|c| c.to_digit(10))
        .map(|d| ONES[d as usize])
        .collect::<Vec<_>>()
        .join(" ")
}

/// `text` with its figures written out: money, thousands, decimals, years
/// and telephone numbers. English only; figures in anything else are left.
pub fn in_words(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len() + 16);
    let mut at = 0;
    while at < chars.len() {
        let c = chars[at];
        let money = match c {
            '$' => Some(("dollar", "dollars", 1)),
            '\u{20b9}' => Some(("rupee", "rupees", 1)),
            '\u{20ac}' => Some(("euro", "euros", 1)),
            '\u{a3}' => Some(("pound", "pounds", 1)),
            'R' | 'r' if at == 0 || !chars[at - 1].is_alphanumeric() => {
                let lead: String = chars[at..].iter().take(3).collect::<String>().to_lowercase();
                (lead.starts_with("rs.") || lead.starts_with("rs "))
                    .then_some(("rupee", "rupees", 3))
            }
            _ => None,
        };
        let mut from = at;
        if let Some((_, _, skip)) = money {
            from += skip;
            while chars.get(from) == Some(&' ') {
                from += 1;
            }
        }
        let starts = chars.get(from).is_some_and(char::is_ascii_digit)
            && (money.is_some() || at == 0 || !chars[at - 1].is_alphanumeric());
        if !starts {
            out.push(c);
            at += 1;
            continue;
        }
        // the figure: digits, thousands' commas, and one point
        let mut end = from;
        let mut point = None;
        while let Some(&d) = chars.get(end) {
            let next = chars.get(end + 1).is_some_and(char::is_ascii_digit);
            match d {
                '0'..='9' => {}
                ',' if next && point.is_none() && end + 2 < chars.len() && chars[end + 2].is_ascii_digit() => {}
                '.' if next && point.is_none() => point = Some(end),
                _ => break,
            }
            end += 1;
        }
        // a time, an ordinal, a date or a model's name is the voice's own
        let tail = chars.get(end).copied().unwrap_or(' ');
        if money.is_none() && (tail.is_alphanumeric() || matches!(tail, ':' | '/' | '-')) {
            while let Some(&d) = chars.get(at) {
                if !(d.is_alphanumeric() || matches!(d, ':' | '/' | '-' | ',' | '.')) {
                    break;
                }
                out.push(d);
                at += 1;
            }
            continue;
        }
        let whole: String = chars[from..point.unwrap_or(end)]
            .iter()
            .filter(|c| c.is_ascii_digit())
            .collect();
        let part: String = point.map(|p| chars[p + 1..end].iter().collect()).unwrap_or_default();
        let comma = chars[from..end].contains(&',');
        let value = whole.parse::<u64>().ok().filter(|_| whole.len() <= 12);
        let (Some(n), false) = (value, whole.len() > 1 && whole.starts_with('0')) else {
            out.push_str(&digits(&whole));
            at = end;
            continue;
        };
        if let Some((one, many, _)) = money {
            out.push_str(&number(n));
            // "$5 million" is five million dollars
            let rest: String = chars[end..].iter().collect();
            let scale = ["million", "billion", "thousand", "lakh", "crore", "trillion"]
                .into_iter()
                .find(|s| rest.trim_start().to_lowercase().starts_with(s));
            if part.len() == 2 && scale.is_none() {
                out.push(' ');
                out.push_str(if n == 1 { one } else { many });
                if let Ok(cents) = part.parse::<u64>() {
                    if cents > 0 {
                        out.push_str(" and ");
                        out.push_str(&number(cents));
                        out.push_str(match one {
                            "rupee" => " paise",
                            "pound" => " pence",
                            _ if cents == 1 => " cent",
                            _ => " cents",
                        });
                    }
                }
                at = end;
            } else {
                if !part.is_empty() {
                    out.push_str(" point ");
                    out.push_str(&digits(&part));
                }
                if let Some(scale) = scale {
                    out.push(' ');
                    out.push_str(scale);
                    at = end + (rest.chars().count() - rest.trim_start().chars().count()) + scale.len();
                } else {
                    at = end;
                }
                out.push(' ');
                out.push_str(if n == 1 && part.is_empty() && scale.is_none() { one } else { many });
            }
            continue;
        }
        if whole.len() >= 7 && !comma && part.is_empty() {
            out.push_str(&digits(&whole));
        } else if whole.len() == 4 && !comma && part.is_empty() && (1100..2100).contains(&n) {
            out.push_str(&year(n));
        } else {
            out.push_str(&number(n));
            if !part.is_empty() {
                out.push_str(" point ");
                out.push_str(&digits(&part));
            }
        }
        at = end;
    }
    out
}

/// Words that give away a language written in Latin letters.
const TELLING: &[(&str, &[&str])] = &[
    ("en", &["the", "and", "is", "you", "of", "to", "in", "it", "for", "that", "are", "this"]),
    ("es", &["el", "los", "las", "una", "qué", "cómo", "está", "por", "para", "muy", "pero", "hola"]),
    ("fr", &["le", "les", "des", "est", "vous", "nous", "je", "une", "pour", "avec", "très", "bonjour"]),
    ("de", &["der", "die", "das", "und", "ist", "ich", "nicht", "ein", "eine", "mit", "wir", "für"]),
    ("pt", &["você", "não", "uma", "são", "os", "com", "muito", "obrigado", "olá", "está", "isso", "em"]),
    ("it", &["il", "gli", "che", "sono", "è", "di", "della", "ciao", "molto", "perché", "questo", "non"]),
    ("id", &["yang", "dan", "saya", "anda", "tidak", "ini", "itu", "untuk", "dengan", "apa", "kamu", "di"]),
];

/// The language `text` is in, as Supertonic names it, by its letters, and
/// Latin ones by a few words that tell; English where none does.
pub fn language(text: &str) -> &'static str {
    let count = |range: std::ops::RangeInclusive<u32>| {
        text.chars().filter(|c| range.contains(&(*c as u32))).count()
    };
    let latin = text.chars().filter(char::is_ascii_alphabetic).count();
    let found = [
        ("ja", count(0x3040..=0x30ff)),
        ("ko", count(0xac00..=0xd7af)),
        ("hi", count(0x0900..=0x097f)),
        ("ar", count(0x0600..=0x06ff)),
        ("el", count(0x0370..=0x03ff)),
        ("ru", count(0x0400..=0x04ff)),
    ];
    match found.into_iter().max_by_key(|(_, n)| *n) {
        Some((name, n)) if n > 0 && n * 2 >= latin => name,
        _ => {
            let words: Vec<String> = pieces(&text.to_lowercase())
                .into_iter()
                .filter(|piece| piece.word)
                .map(|piece| piece.text.to_owned())
                .collect();
            let hits = |list: &[&str]| words.iter().filter(|word| is(word, list)).count();
            let english = hits(TELLING[0].1);
            TELLING[1..]
                .iter()
                .map(|(name, list)| (*name, hits(list)))
                .max_by_key(|(_, n)| *n)
                .filter(|(_, n)| *n >= 2 && *n > english)
                .map_or("en", |(name, _)| name)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_is_said_as_it_is_meant() {
        for text in [
            "the match is broadcasted live",
            "Is it live?",
            "Are you live now?",
            "Watch the live show.",
            "We are live from Mumbai",
            "They streamed it live on Friday",
            "He loves live music",
            "Going live in five minutes",
            "Watch it live tonight",
            "Live from Chennai",
            "This is a live wire",
        ] {
            assert!(respell(text).to_lowercase().contains("lyve"), "{text}: {}", respell(text));
        }
        for text in [
            "the decisions you make determine how you live",
            "Where do you live?",
            "We live in Chennai",
            "I want to live here",
            "You only live once... no, you live twice",
            "They can live with it",
            "People live longer now",
            "Let it live",
            "I now live in Delhi",
            "Live your life",
        ] {
            let said = respell(text).to_lowercase();
            assert!(said.contains("liv ") || said.ends_with("liv") || said.contains("liv?"), "{text}: {said}");
        }
        assert_eq!(respell("Live and lives, alive"), "Liv and lives, alive");
    }

    #[test]
    fn the_rest_are_told_by_their_neighbours() {
        assert_eq!(respell("I read a book yesterday."), "I red a book yesterday.");
        assert_eq!(respell("I like to read books."), "I like to read books.");
        assert_eq!(respell("She has read it"), "She has red it");
        assert_eq!(respell("Lead pipes are heavy; he will lead."), "Led pipes are heavy; he will lead.");
        assert_eq!(respell("Please wind the clock. The wind is strong."), "Please wynd the clock. The wind is strong.");
        assert_eq!(respell("Tear the paper. She shed a tear."), "Tare the paper. She shed a tear.");
        assert_eq!(respell("नमस्ते, आप कैसे हैं?"), "नमस्ते, आप कैसे हैं?");
    }

    #[test]
    fn figures_are_written_out() {
        assert_eq!(in_words("It costs $1,250.50 today."), "It costs one thousand two hundred and fifty dollars and fifty cents today.");
        assert_eq!(in_words("Rs. 500 only"), "five hundred rupees only");
        assert_eq!(in_words("₹1 and $5 million"), "one rupee and five million dollars");
        assert_eq!(in_words("In 2026, 1250 and 10,000"), "In twenty twenty six, twelve fifty and ten thousand");
        assert_eq!(in_words("Call 9876543210."), "Call nine eight seven six five four three two one zero.");
        assert_eq!(in_words("98.6 at 5:30 pm, the 1st, iPhone15, 3/4"), "ninety eight point six at 5:30 pm, the 1st, iPhone15, 3/4");
        assert_eq!(in_words("100% free"), "one hundred% free");
        assert_eq!(in_words("Cars. Hers. 007"), "Cars. Hers. zero zero seven");
    }

    #[test]
    fn the_language_is_told_by_its_letters() {
        assert_eq!(language("Hello there"), "en");
        assert_eq!(language("नमस्ते"), "hi");
        assert_eq!(language("こんにちは"), "ja");
        assert_eq!(language("Привет"), "ru");
        assert_eq!(language("Hola, ¿cómo está? Muy bien"), "es");
        assert_eq!(language("Bonjour, vous êtes très gentil"), "fr");
        assert_eq!(language("The die is cast"), "en");
    }
}
