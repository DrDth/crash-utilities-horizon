//! `ServerConfig`: the original's NeoForge server config, kept as `crashutilities-server.toml` in
//! the game's `config/` folder, read when the server starts.

/// The file, as NeoForge writes it with the defaults when it is missing.
pub const FILE: &str = "crashutilities-server.toml";

/// What NeoForge writes for the original's spec. Its Memory Checker section defines `timer`
/// twice; the second definition replaces the first, so one `timer` holds both.
pub const DEFAULT: &str = r#"
["Item Clear"]
	#Enable regular ItemClear
	enabled = false
	#Waiting time between Checks in Minutes
	# Default: 30
	# Range: 0 ~ 1440
	timer = 30
	#Do a clear if there are more than X items
	# Default: 1000
	# Range: > 0
	maximum = 1000
	#Chat warning, Comma separated, in seconds
	warnings = "5,20"
	#Run a Title command on first warning
	title = true
	#Text to display in TitleCommand
	titletext = "ITEMCLEAR INCOMING!"
	#Text to Display in Chat when Item. First % will be replaced by Value. In red, encased in [===
	warningtext = "ITEMCLEAR IN % SECONDS"

["Memory Checker"]
	#Enable the Memory checker
	enabled = false
	#Time between Memory Checks that will get saved for display(in seconds)
	# Default: 30
	# Range: > 0
	timer = 30
	#Threshold at which the Memory checker will display a warning in the Log (in MB)
	# Default: 1000
	# Range: > 0
	threshold = 1000
	#Run /spark heapdump when memory fills up more than 95 % the first time. This value gets set to false if this occurs
	heapdump = false

["Ftb Chunks"]
	#If Ftb chunks is installed enable automatic purge of loaded chunks for people who have not been online for some amount of days. (Note LOADED not Claimed Chunks)
	enabled = false
	#Number of days after which a players chunks will be unloaded. Warning! This relies on the modify date of the player data file.
	# Default: 7
	# Range: > 0
	days = 7
"#;

/// The values the port reads.
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    /// `Item Clear.enabled`.
    pub item_clear: bool,
    /// `Item Clear.timer`: minutes between checks, 0 for ten seconds.
    pub timer: i32,
    /// `Item Clear.maximum`.
    pub maximum: i32,
    /// `Item Clear.warnings`, in seconds before the clear, ascending; None when a part isn't a
    /// number, where the original's `Integer.parseInt` throws and no clear runs.
    pub warnings: Option<Vec<i32>>,
    /// `Item Clear.title`.
    pub title: bool,
    /// `Item Clear.titletext`.
    pub title_text: String,
    /// `Item Clear.warningtext`.
    pub warning_text: String,
}

impl Default for Config {
    fn default() -> Self {
        Config::parse(DEFAULT)
    }
}

/// One `key = value` of the file, under its section.
struct Entry<'a> {
    section: String,
    key: &'a str,
    value: &'a str,
}

/// The entries of a TOML file as NeoForge writes it: `["Section"]` headers, `key = value` lines
/// and `#` comments.
fn entries(text: &str) -> Vec<Entry<'_>> {
    let mut section = String::new();
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(header) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            section = header.trim().trim_matches('"').to_owned();
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            out.push(Entry {
                section: section.clone(),
                key: key.trim(),
                value: without_comment(value.trim()),
            });
        }
    }
    out
}

/// The value without a trailing `# comment` outside its quotes.
fn without_comment(value: &str) -> &str {
    let mut quoted = false;
    let mut escaped = false;
    for (i, c) in value.char_indices() {
        match c {
            '\\' if quoted && !escaped => {
                escaped = true;
                continue;
            }
            '"' if !escaped => quoted = !quoted,
            '#' if !quoted => return value[..i].trim_end(),
            _ => {}
        }
        escaped = false;
    }
    value
}

/// A basic TOML string's contents.
fn string(value: &str) -> Option<String> {
    let inner = value.strip_prefix('"')?.strip_suffix('"')?;
    let mut out = String::new();
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next()? {
            'n' => out.push('\n'),
            't' => out.push('\t'),
            c @ ('"' | '\\') => out.push(c),
            _ => return None,
        }
    }
    Some(out)
}

impl Config {
    /// The config from the file's text. A value that is missing, of the wrong type or out of its
    /// range takes its default, as NeoForge corrects it.
    pub fn parse(text: &str) -> Config {
        let entries = entries(text);
        let get = |section: &str, key: &str| {
            entries
                .iter()
                .rev()
                .find(|e| e.section == section && e.key == key)
                .map(|e| e.value)
        };
        let boolean = |key: &str, default: bool| match get("Item Clear", key) {
            Some("true") => true,
            Some("false") => false,
            _ => default,
        };
        let int = |key: &str, default: i32, min: i32, max: i32| {
            get("Item Clear", key)
                .and_then(|v| v.parse::<i32>().ok())
                .filter(|v| (min..=max).contains(v))
                .unwrap_or(default)
        };
        let text = |key: &str, default: &str| {
            get("Item Clear", key)
                .and_then(string)
                .unwrap_or_else(|| default.to_owned())
        };
        let warnings = text("warnings", "5,20");
        Config {
            item_clear: boolean("enabled", false),
            timer: int("timer", 30, 0, 1440),
            maximum: int("maximum", 1000, 0, i32::MAX),
            warnings: warnings_of(&warnings),
            title: boolean("title", true),
            title_text: text("titletext", "ITEMCLEAR INCOMING!"),
            warning_text: text("warningtext", "ITEMCLEAR IN % SECONDS"),
        }
    }
}

/// `ServerConfig.getWarnings`: the comma-separated seconds, sorted; None when one isn't an
/// `int` as `Integer.parseInt` reads it (no spaces).
fn warnings_of(text: &str) -> Option<Vec<i32>> {
    let mut out = text
        .split(',')
        .map(|part| {
            let digits = part.strip_prefix(['+', '-']).unwrap_or(part);
            (!digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
                .then(|| part.parse::<i32>().ok())
                .flatten()
        })
        .collect::<Option<Vec<_>>>()?;
    out.sort_unstable();
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_originals() {
        let config = Config::default();
        assert!(!config.item_clear);
        assert_eq!((config.timer, config.maximum), (30, 1000));
        assert_eq!(config.warnings, Some(vec![5, 20]));
        assert!(config.title);
        assert_eq!(config.title_text, "ITEMCLEAR INCOMING!");
        assert_eq!(config.warning_text, "ITEMCLEAR IN % SECONDS");
    }

    #[test]
    fn values_are_read_by_section_and_corrected_out_of_range() {
        let config = Config::parse(
            "[\"Memory Checker\"]\nenabled = true\ntimer = 3\n\
             [\"Item Clear\"]\n\tenabled = true # on\n\ttimer = 5000\n\tmaximum = 10\n\
             \twarnings = \"30,1,10\"\n\twarningtext = \"CLEAR \\\"%\\\" #1\"\n",
        );
        assert!(config.item_clear);
        assert_eq!(config.timer, 30, "past 1440 the default");
        assert_eq!(config.maximum, 10);
        assert_eq!(config.warnings, Some(vec![1, 10, 30]));
        assert_eq!(config.warning_text, "CLEAR \"%\" #1");
    }

    #[test]
    fn a_warning_that_is_no_int_stops_the_clear() {
        assert_eq!(warnings_of("5, 20"), None);
        assert_eq!(warnings_of(""), None);
        assert_eq!(warnings_of("+5,-1"), Some(vec![-1, 5]));
    }
}
