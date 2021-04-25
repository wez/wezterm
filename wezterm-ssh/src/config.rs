//! Parse an ssh_config(5) formatted config file
use regex::{Captures, Regex};
use std::collections::BTreeMap;
use std::path::Path;

pub type ConfigMap = BTreeMap<String, String>;

/// A Pattern in a `Host` list
#[derive(Debug, PartialEq, Eq, Clone)]
struct Pattern {
    negated: bool,
    pattern: String,
}

/// Compile a glob style pattern string into a regex pattern string
fn wildcard_to_pattern(s: &str) -> String {
    let mut pattern = String::new();
    pattern.push('^');
    for c in s.chars() {
        if c == '*' {
            pattern.push_str(".*");
        } else if c == '?' {
            pattern.push('.');
        } else {
            let s = regex::escape(&c.to_string());
            pattern.push_str(&s);
        }
    }
    pattern.push('$');
    pattern
}

impl Pattern {
    /// Returns true if this pattern matches the provided hostname
    fn match_text(&self, hostname: &str) -> bool {
        if let Ok(re) = Regex::new(&self.pattern) {
            re.is_match(hostname)
        } else {
            false
        }
    }

    fn new(text: &str, negated: bool) -> Self {
        Self {
            pattern: wildcard_to_pattern(text),
            negated,
        }
    }
}

/// Represents `Host patter,list` stanza in the config,
/// and the options that it logically contains
#[derive(Debug, PartialEq, Eq, Clone)]
struct HostGroup {
    patterns: Vec<Pattern>,
    options: ConfigMap,
}

impl HostGroup {
    /// Returns true if hostname matches this stanza
    fn is_match(&self, hostname: &str) -> bool {
        for pat in &self.patterns {
            if pat.match_text(hostname) {
                // We got a definitive name match.
                // If it was an exlusion then we've been told
                // that this doesn't really match, otherwise
                // we got one that we were looking for
                return !pat.negated;
            }
        }
        false
    }
}

/// Holds the ordered set of parsed options.
/// The config file semantics are that the first matching value
/// for a given option takes precedence
#[derive(Debug, PartialEq, Eq, Clone)]
struct ParsedConfigFile {
    /// options that appeared before any `Host` stanza
    options: ConfigMap,
    /// options inside a `Host` stanza
    groups: Vec<HostGroup>,
}

impl ParsedConfigFile {
    fn parse(s: &str) -> Self {
        let mut options = ConfigMap::new();
        let mut groups = vec![];

        for line in s.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Some(sep) = line
                .find('=')
                .or_else(|| line.find(|c: char| c.is_whitespace()))
            {
                let (k, v) = line.split_at(sep);
                let k = k.trim().to_lowercase();
                let v = v[1..].trim();

                let v = if v.starts_with('"') && v.ends_with('"') {
                    &v[1..v.len() - 1]
                } else {
                    v
                };

                if k == "host" {
                    let mut patterns = vec![];
                    for p in v.split(',') {
                        let p = p.trim();
                        if p.starts_with('!') {
                            patterns.push(Pattern::new(&p[1..], true));
                        } else {
                            patterns.push(Pattern::new(p, false));
                        }
                    }
                    groups.push(HostGroup {
                        patterns,
                        options: ConfigMap::new(),
                    });
                    continue;
                }

                if let Some(group) = groups.last_mut() {
                    group
                        .options
                        .entry(k.to_string())
                        .or_insert_with(|| v.to_string());
                } else {
                    options
                        .entry(k.to_string())
                        .or_insert_with(|| v.to_string());
                }
            }
        }

        Self { options, groups }
    }

    /// Apply configuration values that match the specified hostname to target,
    /// but only if a given key is not already present in target, because the
    /// semantics are that the first match wins
    fn apply_matches(&self, hostname: &str, target: &mut ConfigMap) {
        for (k, v) in &self.options {
            target.entry(k.to_string()).or_insert_with(|| v.to_string());
        }
        for group in &self.groups {
            if group.is_match(hostname) {
                for (k, v) in &group.options {
                    target.entry(k.to_string()).or_insert_with(|| v.to_string());
                }
            }
        }
    }
}

/// A context for resolving configuration values.
/// Holds a combination of environment and token expansion state,
/// as well as the set of configs that should be consulted.
#[derive(Debug, Clone)]
pub struct Config {
    config_files: Vec<ParsedConfigFile>,
    options: ConfigMap,
    tokens: ConfigMap,
    environment: Option<ConfigMap>,
}

impl Config {
    /// Create a new context without any config files loaded
    pub fn new() -> Self {
        Self {
            config_files: vec![],
            options: ConfigMap::new(),
            tokens: ConfigMap::new(),
            environment: None,
        }
    }

    /// Assign a fake environment map, useful for testing.
    /// The environment is used to expand certain values
    /// from the config.
    pub fn assign_environment(&mut self, env: ConfigMap) {
        self.environment.replace(env);
    }

    /// Assigns token names and expansions for use with a number of
    /// options.  The names and expansions are specified
    /// by `man 5 ssh_config`
    pub fn assign_tokens(&mut self, tokens: ConfigMap) {
        self.tokens = tokens;
    }

    /// Assign the value for an option.
    /// This is logically equivalent to the user specifying command
    /// line options to override config values.
    /// These values take precedence over any values found in config files.
    pub fn set_option<K: AsRef<str>, V: AsRef<str>>(&mut self, key: K, value: V) {
        self.options
            .insert(key.as_ref().to_lowercase(), value.as_ref().to_string());
    }

    /// Parse `config_string` as if it were the contents of an `ssh_config` file,
    /// and add that to the list of configs.
    pub fn add_config_string(&mut self, config_string: &str) {
        self.config_files
            .push(ParsedConfigFile::parse(config_string));
    }

    /// Open `path`, read its contents and parse it as an `ssh_config` file,
    /// adding that to the list of configs
    pub fn add_config_file<P: AsRef<Path>>(&mut self, path: P) {
        if let Ok(data) = std::fs::read_to_string(path) {
            self.config_files.push(ParsedConfigFile::parse(&data));
        }
    }

    /// Convenience method for adding the ~/.ssh/config and system-wide
    /// `/etc/ssh/config` files to the list of configs
    pub fn add_default_config_files(&mut self) {
        if let Some(home) = dirs_next::home_dir() {
            self.add_config_file(home.join(".ssh").join("config"));
        }
        self.add_config_file("/etc/ssh/config");
    }

    /// Resolve the configuration for a given host.
    /// The returned map will expand environment and tokens for options
    /// where that is specified.
    /// Note that in some configurations, the config should be parsed once
    /// to resolve the main configuration, and then based on some options
    /// (such as CanonicalHostname), the tokens should be updated and
    /// the config parsed a second time in order for value expansion
    /// to have the same results as `ssh`.
    pub fn for_host<H: AsRef<str>>(&self, host: H) -> ConfigMap {
        let host = host.as_ref();
        let mut result = self.options.clone();

        for config in &self.config_files {
            config.apply_matches(host, &mut result);
        }

        for (k, v) in &mut result {
            if let Some(tokens) = self.should_expand_tokens(k) {
                self.expand_tokens(v, tokens);
            }

            if self.should_expand_environment(k) {
                self.expand_environment(v);
            }
        }

        result
            .entry("hostname".to_string())
            .or_insert_with(|| host.to_string());

        result
            .entry("port".to_string())
            .or_insert_with(|| "22".to_string());

        result.entry("user".to_string()).or_insert_with(|| {
            for user in &["USER", "USERNAME"] {
                if let Some(user) = self.resolve_env(user) {
                    return user;
                }
            }
            "unknown-user".to_string()
        });

        if !result.contains_key("userknownhostsfile") {
            if let Some(home) = self.resolve_home() {
                result.insert(
                    "userknownhostsfile".to_string(),
                    format!("{}/.ssh/known_hosts {}/.ssh/known_hosts2", home, home,),
                );
            }
        }

        if !result.contains_key("identityfile") {
            if let Some(home) = self.resolve_home() {
                result.insert(
                    "identityfile".to_string(),
                    format!(
                        "{}/.ssh/id_dsa {}/.ssh/id_ecdsa {}/.ssh/id_ed25519 {}/.ssh/id_rsa",
                        home, home, home, home
                    ),
                );
            }
        }

        if !result.contains_key("identityagent") {
            if let Some(sock_path) = self.resolve_env("SSH_AUTH_SOCK") {
                result.insert("identityagent".to_string(), sock_path);
            }
        }

        result
    }

    /// Return true if a given option name is subject to environment variable
    /// expansion.
    fn should_expand_environment(&self, key: &str) -> bool {
        match key {
            "certificatefile" | "controlpath" | "identityagent" | "identityfile"
            | "userknownhostsfile" | "localforward" | "remoteforward" => true,
            _ => false,
        }
    }

    /// Returns a set of tokens that should be expanded for a given option name
    fn should_expand_tokens(&self, key: &str) -> Option<&[&str]> {
        match key {
            "certificatefile" | "controlpath" | "identityagent" | "identityfile"
            | "localforward" | "remotecommand" | "remoteforward" | "userknownkostsfile" => {
                Some(&["%C", "%d", "%h", "%i", "%L", "%l", "%n", "%p", "%r", "%u"])
            }
            "hostname" => Some(&["%h"]),
            "localcommand" => Some(&[
                "%C", "%d", "%h", "%i", "%k", "%L", "%l", "%n", "%p", "%r", "%T", "%u",
            ]),
            "proxycommand" => Some(&["%h", "%n", "%p", "%r"]),
            _ => None,
        }
    }

    /// Resolve the home directory.
    /// For the sake of unit testing, this will look for HOME in the provided
    /// environment override before asking the system for the home directory.
    fn resolve_home(&self) -> Option<String> {
        if let Some(env) = self.environment.as_ref() {
            if let Some(home) = env.get("HOME") {
                return Some(home.to_string());
            }
        }
        if let Some(home) = dirs_next::home_dir() {
            if let Some(home) = home.to_str() {
                return Some(home.to_string());
            }
        }
        None
    }

    /// Perform token substitution
    fn expand_tokens(&self, value: &mut String, tokens: &[&str]) {
        for &t in tokens {
            if let Some(v) = self.tokens.get(t) {
                *value = value.replace(t, v);
            } else if t == "%d" {
                if let Some(home) = self.resolve_home() {
                    if value.starts_with("~/") {
                        value.replace_range(0..1, &home);
                    } else {
                        *value = value.replace(t, &home);
                    }
                }
            }
        }

        *value = value.replace("%%", "%");
    }

    /// Resolve an environment variable; if an override is set use that,
    /// otherwise read from the real environment.
    fn resolve_env(&self, name: &str) -> Option<String> {
        if let Some(env) = self.environment.as_ref() {
            env.get(name).cloned()
        } else {
            std::env::var(name).ok()
        }
    }

    /// Look for `${NAME}` and substitute the value of the `NAME` env var
    /// into the provided string.
    fn expand_environment(&self, value: &mut String) {
        let re = Regex::new(r#"\$\{([a-zA-Z_][a-zA-Z_0-9]+)\}"#).unwrap();
        *value = re
            .replace_all(value, |caps: &Captures| -> String {
                if let Some(rep) = self.resolve_env(&caps[1]) {
                    rep
                } else {
                    caps[0].to_string()
                }
            })
            .to_string();
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use k9::snapshot;

    #[test]
    fn parse_user() {
        let mut config = Config::new();

        let mut fake_env = ConfigMap::new();
        fake_env.insert("HOME".to_string(), "/home/me".to_string());
        fake_env.insert("USER".to_string(), "me".to_string());
        config.assign_environment(fake_env);

        config.add_config_string(
            r#"
        Host foo
            HostName 10.0.0.1
            User foo
            IdentityFile "%d/.ssh/id_pub.dsa"
            "#,
        );

        snapshot!(
            &config,
            r#"
Config {
    config_files: [
        ParsedConfigFile {
            options: {},
            groups: [
                HostGroup {
                    patterns: [
                        Pattern {
                            negated: false,
                            pattern: "^foo$",
                        },
                    ],
                    options: {
                        "hostname": "10.0.0.1",
                        "identityfile": "%d/.ssh/id_pub.dsa",
                        "user": "foo",
                    },
                },
            ],
        },
    ],
    options: {},
    tokens: {},
    environment: Some(
        {
            "HOME": "/home/me",
            "USER": "me",
        },
    ),
}
"#
        );

        let opts = config.for_host("foo");
        snapshot!(
            opts,
            r#"
{
    "hostname": "10.0.0.1",
    "identityfile": "/home/me/.ssh/id_pub.dsa",
    "port": "22",
    "user": "foo",
    "userknownhostsfile": "/home/me/.ssh/known_hosts /home/me/.ssh/known_hosts2",
}
"#
        );
    }

    #[test]
    fn sub_tilde() {
        let mut config = Config::new();

        let mut fake_env = ConfigMap::new();
        fake_env.insert("HOME".to_string(), "/home/me".to_string());
        fake_env.insert("USER".to_string(), "me".to_string());
        config.assign_environment(fake_env);

        config.add_config_string(
            r#"
        Host foo
            HostName 10.0.0.1
            User foo
            IdentityFile "~/.ssh/id_pub.dsa"
            "#,
        );

        let opts = config.for_host("foo");
        snapshot!(
            opts,
            r#"
{
    "hostname": "10.0.0.1",
    "identityfile": "/home/me/.ssh/id_pub.dsa",
    "port": "22",
    "user": "foo",
    "userknownhostsfile": "/home/me/.ssh/known_hosts /home/me/.ssh/known_hosts2",
}
"#
        );
    }

    #[test]
    fn parse_simple() {
        let mut config = Config::new();

        let mut fake_env = ConfigMap::new();
        fake_env.insert("HOME".to_string(), "/home/me".to_string());
        fake_env.insert("USER".to_string(), "me".to_string());
        config.assign_environment(fake_env);

        config.add_config_string(
            r#"
        # I am a comment
        Something first
        # the prior Something takes precedence
        Something ignored
        Host 192.168.1.8,wopr
            FowardAgent yes
            IdentityFile "%d/.ssh/id_pub.dsa"

        Host !a.b,*.b
            ForwardAgent no
            IdentityAgent "${HOME}/.ssh/agent"

        Host *
            Something  else
            "#,
        );

        snapshot!(
            &config,
            r#"
Config {
    config_files: [
        ParsedConfigFile {
            options: {
                "something": "first",
            },
            groups: [
                HostGroup {
                    patterns: [
                        Pattern {
                            negated: false,
                            pattern: "^192\\.168\\.1\\.8$",
                        },
                        Pattern {
                            negated: false,
                            pattern: "^wopr$",
                        },
                    ],
                    options: {
                        "fowardagent": "yes",
                        "identityfile": "%d/.ssh/id_pub.dsa",
                    },
                },
                HostGroup {
                    patterns: [
                        Pattern {
                            negated: true,
                            pattern: "^a\\.b$",
                        },
                        Pattern {
                            negated: false,
                            pattern: "^.*\\.b$",
                        },
                    ],
                    options: {
                        "forwardagent": "no",
                        "identityagent": "${HOME}/.ssh/agent",
                    },
                },
                HostGroup {
                    patterns: [
                        Pattern {
                            negated: false,
                            pattern: "^.*$",
                        },
                    ],
                    options: {
                        "something": "else",
                    },
                },
            ],
        },
    ],
    options: {},
    tokens: {},
    environment: Some(
        {
            "HOME": "/home/me",
            "USER": "me",
        },
    ),
}
"#
        );

        let opts = config.for_host("random");
        snapshot!(
            opts,
            r#"
{
    "hostname": "random",
    "identityfile": "/home/me/.ssh/id_dsa /home/me/.ssh/id_ecdsa /home/me/.ssh/id_ed25519 /home/me/.ssh/id_rsa",
    "port": "22",
    "something": "first",
    "user": "me",
    "userknownhostsfile": "/home/me/.ssh/known_hosts /home/me/.ssh/known_hosts2",
}
"#
        );

        let opts = config.for_host("192.168.1.8");
        snapshot!(
            opts,
            r#"
{
    "fowardagent": "yes",
    "hostname": "192.168.1.8",
    "identityfile": "/home/me/.ssh/id_pub.dsa",
    "port": "22",
    "something": "first",
    "user": "me",
    "userknownhostsfile": "/home/me/.ssh/known_hosts /home/me/.ssh/known_hosts2",
}
"#
        );

        let opts = config.for_host("a.b");
        snapshot!(
            opts,
            r#"
{
    "hostname": "a.b",
    "identityfile": "/home/me/.ssh/id_dsa /home/me/.ssh/id_ecdsa /home/me/.ssh/id_ed25519 /home/me/.ssh/id_rsa",
    "port": "22",
    "something": "first",
    "user": "me",
    "userknownhostsfile": "/home/me/.ssh/known_hosts /home/me/.ssh/known_hosts2",
}
"#
        );

        let opts = config.for_host("b.b");
        snapshot!(
            opts,
            r#"
{
    "forwardagent": "no",
    "hostname": "b.b",
    "identityagent": "/home/me/.ssh/agent",
    "identityfile": "/home/me/.ssh/id_dsa /home/me/.ssh/id_ecdsa /home/me/.ssh/id_ed25519 /home/me/.ssh/id_rsa",
    "port": "22",
    "something": "first",
    "user": "me",
    "userknownhostsfile": "/home/me/.ssh/known_hosts /home/me/.ssh/known_hosts2",
}
"#
        );
    }
}
