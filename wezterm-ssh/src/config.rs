//! Parse an ssh_config(5) formatted config file
use regex::{Captures, Regex};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub type ConfigMap = BTreeMap<String, String>;

/// A Pattern in a `Host` list
#[derive(Debug, PartialEq, Eq, Clone)]
struct Pattern {
    negated: bool,
    pattern: String,
    original: String,
    is_literal: bool,
}

/// Compile a glob style pattern string into a regex pattern string
fn wildcard_to_pattern(s: &str) -> (String, bool) {
    let mut pattern = String::new();
    let mut is_literal = true;
    pattern.push('^');
    for c in s.chars() {
        if c == '*' {
            pattern.push_str(".*");
            is_literal = false;
        } else if c == '?' {
            pattern.push('.');
            is_literal = false;
        } else {
            let s = regex::escape(&c.to_string());
            pattern.push_str(&s);
        }
    }
    pattern.push('$');
    (pattern, is_literal)
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
        let (pattern, is_literal) = wildcard_to_pattern(text);
        Self {
            pattern,
            is_literal,
            negated,
            original: text.to_string(),
        }
    }

    /// Returns true if hostname matches the
    /// condition specified by a list of patterns
    fn match_group(hostname: &str, patterns: &[Self]) -> bool {
        for pat in patterns {
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

#[derive(Clone, Eq, PartialEq, Debug)]
enum Criteria {
    Host(Vec<Pattern>),
    Exec(String),
    OriginalHost(Vec<Pattern>),
    User(Vec<Pattern>),
    LocalUser(Vec<Pattern>),
    All,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
enum Context {
    FirstPass,
    Canonical,
    Final,
}

/// Represents `Host pattern,list` stanza in the config,
/// and the options that it logically contains
#[derive(Debug, PartialEq, Eq, Clone)]
struct MatchGroup {
    criteria: Vec<Criteria>,
    context: Context,
    options: ConfigMap,
}

impl MatchGroup {
    fn is_match(&self, hostname: &str, user: &str, local_user: &str, context: Context) -> bool {
        if self.context != context {
            return false;
        }
        for c in &self.criteria {
            match c {
                Criteria::Host(patterns) => {
                    if !Pattern::match_group(hostname, patterns) {
                        return false;
                    }
                }
                Criteria::Exec(_) => {
                    log::warn!("Match Exec is not implemented");
                }
                Criteria::OriginalHost(patterns) => {
                    if !Pattern::match_group(hostname, patterns) {
                        return false;
                    }
                }
                Criteria::User(patterns) => {
                    if !Pattern::match_group(user, patterns) {
                        return false;
                    }
                }
                Criteria::LocalUser(patterns) => {
                    if !Pattern::match_group(local_user, patterns) {
                        return false;
                    }
                }
                Criteria::All => {
                    // Always matches
                }
            }
        }
        true
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
    groups: Vec<MatchGroup>,
    /// list of loaded file names
    loaded_files: Vec<PathBuf>,
}

impl ParsedConfigFile {
    fn parse(s: &str, cwd: Option<&Path>, source_file: Option<&Path>) -> Self {
        let mut options = ConfigMap::new();
        let mut groups = vec![];
        let mut loaded_files = vec![];

        if let Some(source) = source_file {
            loaded_files.push(source.to_path_buf());
        }

        Self::parse_impl(s, cwd, &mut options, &mut groups, &mut loaded_files);

        Self {
            options,
            groups,
            loaded_files,
        }
    }

    fn do_include(
        pattern: &str,
        cwd: Option<&Path>,
        options: &mut ConfigMap,
        groups: &mut Vec<MatchGroup>,
        loaded_files: &mut Vec<PathBuf>,
    ) {
        match filenamegen::Glob::new(&pattern) {
            Ok(g) => {
                match cwd
                    .as_ref()
                    .map(|p| p.to_path_buf())
                    .or_else(|| std::env::current_dir().ok())
                {
                    Some(cwd) => {
                        for path in g.walk(&cwd) {
                            let path = if path.is_absolute() {
                                path
                            } else {
                                cwd.join(path)
                            };
                            match std::fs::read_to_string(&path) {
                                Ok(data) => {
                                    loaded_files.push(path.clone());
                                    Self::parse_impl(
                                        &data,
                                        Some(&cwd),
                                        options,
                                        groups,
                                        loaded_files,
                                    );
                                }
                                Err(err) => {
                                    log::error!(
                                        "error expanding `Include {}`: unable to open {}: {:#}",
                                        pattern,
                                        path.display(),
                                        err
                                    );
                                }
                            }
                        }
                    }
                    None => {
                        log::error!(
                            "error expanding `Include {}`: unable to determine cwd",
                            pattern
                        );
                    }
                }
            }
            Err(err) => {
                log::error!("error expanding `Include {}`: {:#}", pattern, err);
            }
        }
    }

    fn parse_impl(
        s: &str,
        cwd: Option<&Path>,
        options: &mut ConfigMap,
        groups: &mut Vec<MatchGroup>,
        loaded_files: &mut Vec<PathBuf>,
    ) {
        for line in s.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Some(sep) = line.find(|c: char| c == '=' || c.is_whitespace()) {
                let (k, v) = line.split_at(sep);
                let k = k.trim().to_lowercase();
                let v = v[1..].trim();

                let v = if v.starts_with('"') && v.ends_with('"') {
                    &v[1..v.len() - 1]
                } else {
                    v
                };

                fn parse_pattern_list(v: &str) -> Vec<Pattern> {
                    let mut patterns = vec![];
                    for p in v.split(',') {
                        let p = p.trim();
                        if p.starts_with('!') {
                            patterns.push(Pattern::new(&p[1..], true));
                        } else {
                            patterns.push(Pattern::new(p, false));
                        }
                    }
                    patterns
                }
                fn parse_whitespace_pattern_list(v: &str) -> Vec<Pattern> {
                    let mut patterns = vec![];
                    for p in v.split_ascii_whitespace() {
                        let p = p.trim();
                        if p.starts_with('!') {
                            patterns.push(Pattern::new(&p[1..], true));
                        } else {
                            patterns.push(Pattern::new(p, false));
                        }
                    }
                    patterns
                }

                if k == "include" {
                    Self::do_include(v, cwd, options, groups, loaded_files);
                    continue;
                }

                if k == "host" {
                    let patterns = parse_whitespace_pattern_list(v);
                    groups.push(MatchGroup {
                        criteria: vec![Criteria::Host(patterns)],
                        options: ConfigMap::new(),
                        context: Context::FirstPass,
                    });
                    continue;
                }

                if k == "match" {
                    let mut criteria = vec![];
                    let mut context = Context::FirstPass;

                    let mut tokens = v.split_ascii_whitespace();

                    while let Some(cname) = tokens.next() {
                        match cname.to_lowercase().as_str() {
                            "all" => {
                                criteria.push(Criteria::All);
                            }
                            "canonical" => {
                                context = Context::Canonical;
                            }
                            "final" => {
                                context = Context::Final;
                            }
                            "exec" => {
                                criteria.push(Criteria::Exec(
                                    tokens.next().unwrap_or("false").to_string(),
                                ));
                            }
                            "host" => {
                                criteria.push(Criteria::Host(parse_pattern_list(
                                    tokens.next().unwrap_or(""),
                                )));
                            }
                            "originalhost" => {
                                criteria.push(Criteria::OriginalHost(parse_pattern_list(
                                    tokens.next().unwrap_or(""),
                                )));
                            }
                            "user" => {
                                criteria.push(Criteria::User(parse_pattern_list(
                                    tokens.next().unwrap_or(""),
                                )));
                            }
                            "localuser" => {
                                criteria.push(Criteria::LocalUser(parse_pattern_list(
                                    tokens.next().unwrap_or(""),
                                )));
                            }
                            _ => break,
                        }
                    }

                    groups.push(MatchGroup {
                        criteria,
                        options: ConfigMap::new(),
                        context,
                    });
                    continue;
                }

                fn add_option(options: &mut ConfigMap, k: String, v: &str) {
                    // first option wins in ssh_config, except for identityfile
                    // which explicitly allows multiple entries to combine together
                    let is_identity_file = k == "identityfile";
                    options
                        .entry(k)
                        .and_modify(|e| {
                            if is_identity_file {
                                e.push(' ');
                                e.push_str(v);
                            }
                        })
                        .or_insert_with(|| v.to_string());
                }

                if let Some(group) = groups.last_mut() {
                    add_option(&mut group.options, k, v);
                } else {
                    add_option(options, k, v);
                }
            }
        }
    }

    /// Apply configuration values that match the specified hostname to target,
    /// but only if a given key is not already present in target, because the
    /// semantics are that the first match wins
    fn apply_matches(
        &self,
        hostname: &str,
        user: &str,
        local_user: &str,
        context: Context,
        target: &mut ConfigMap,
    ) -> bool {
        let mut needs_reparse = false;

        for (k, v) in &self.options {
            target.entry(k.to_string()).or_insert_with(|| v.to_string());
        }
        for group in &self.groups {
            if group.context != Context::FirstPass {
                needs_reparse = true;
            }
            if group.is_match(hostname, user, local_user, context) {
                for (k, v) in &group.options {
                    target.entry(k.to_string()).or_insert_with(|| v.to_string());
                }
            }
        }

        needs_reparse
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
            .push(ParsedConfigFile::parse(config_string, None, None));
    }

    /// Open `path`, read its contents and parse it as an `ssh_config` file,
    /// adding that to the list of configs
    pub fn add_config_file<P: AsRef<Path>>(&mut self, path: P) {
        if let Ok(data) = std::fs::read_to_string(path.as_ref()) {
            self.config_files.push(ParsedConfigFile::parse(
                &data,
                path.as_ref().parent(),
                Some(path.as_ref()),
            ));
        }
    }

    /// Convenience method for adding the ~/.ssh/config and system-wide
    /// `/etc/ssh/config` files to the list of configs
    pub fn add_default_config_files(&mut self) {
        if let Some(home) = dirs_next::home_dir() {
            self.add_config_file(home.join(".ssh").join("config"));
        }
        self.add_config_file("/etc/ssh/ssh_config");
        if let Ok(sysdrive) = std::env::var("SystemDrive") {
            self.add_config_file(format!("{}/ProgramData/ssh/ssh_config", sysdrive));
        }
    }

    fn resolve_local_host(&self, include_domain_name: bool) -> String {
        let hostname = gethostname::gethostname().to_string_lossy().to_string();

        if include_domain_name {
            hostname
        } else {
            match hostname.split_once('.') {
                Some((hostname, _domain)) => hostname.to_string(),
                None => hostname,
            }
        }
    }

    fn resolve_local_user(&self) -> String {
        for user in &["USER", "USERNAME"] {
            if let Some(user) = self.resolve_env(user) {
                return user;
            }
        }
        "unknown-user".to_string()
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
        let local_user = self.resolve_local_user();
        let target_user = &local_user;

        let mut result = self.options.clone();
        let mut needs_reparse = false;

        for config in &self.config_files {
            if config.apply_matches(
                host,
                target_user,
                &local_user,
                Context::FirstPass,
                &mut result,
            ) {
                needs_reparse = true;
            }
        }

        if needs_reparse {
            log::debug!(
                "ssh configuration uses options that require two-phase \
                parsing, which isn't supported"
            );
        }

        let mut token_map = self.tokens.clone();
        token_map.insert("%h".to_string(), host.to_string());
        result
            .entry("hostname".to_string())
            .and_modify(|curr| {
                if let Some(tokens) = self.should_expand_tokens("hostname") {
                    self.expand_tokens(curr, tokens, &token_map);
                }
            })
            .or_insert_with(|| host.to_string());
        token_map.insert("%h".to_string(), result["hostname"].to_string());
        token_map.insert("%n".to_string(), host.to_string());
        token_map.insert("%r".to_string(), target_user.to_string());
        token_map.insert(
            "%p".to_string(),
            result
                .get("port")
                .map(|p| p.to_string())
                .unwrap_or_else(|| "22".to_string()),
        );

        for (k, v) in &mut result {
            if let Some(tokens) = self.should_expand_tokens(k) {
                self.expand_tokens(v, tokens, &token_map);
            }

            if self.should_expand_environment(k) {
                self.expand_environment(v);
            }
        }

        result
            .entry("port".to_string())
            .or_insert_with(|| "22".to_string());

        result
            .entry("user".to_string())
            .or_insert_with(|| target_user.clone());

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
    fn expand_tokens(&self, value: &mut String, tokens: &[&str], token_map: &ConfigMap) {
        let orig_value = value.to_string();
        for &t in tokens {
            if let Some(v) = token_map.get(t) {
                *value = value.replace(t, v);
            } else if t == "%u" {
                *value = value.replace(t, &self.resolve_local_user());
            } else if t == "%l" {
                *value = value.replace(t, &self.resolve_local_host(false));
            } else if t == "%L" {
                *value = value.replace(t, &self.resolve_local_host(true));
            } else if t == "%d" {
                if let Some(home) = self.resolve_home() {
                    let mut items = value
                        .split_whitespace()
                        .map(|s| s.to_string())
                        .collect::<Vec<String>>();
                    for item in &mut items {
                        if item.starts_with("~/") {
                            item.replace_range(0..1, &home);
                        } else {
                            *item = item.replace(t, &home);
                        }
                    }
                    *value = items.join(" ");
                }
            } else if value.contains(t) {
                log::warn!("Unsupported token {t} when evaluating `{orig_value}`");
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

    /// Returns the list of file names that were loaded as part of parsing
    /// the ssh config
    pub fn loaded_config_files(&self) -> Vec<PathBuf> {
        let mut files = vec![];

        for config in &self.config_files {
            for file in &config.loaded_files {
                if !files.contains(file) {
                    files.push(file.to_path_buf());
                }
            }
        }

        files
    }

    /// Returns the list of host names that have defined ssh config entries.
    /// The host names are literal (non-pattern), non-negated hosts extracted
    /// from `Host` and `Match` stanzas in the ssh config.
    pub fn enumerate_hosts(&self) -> Vec<String> {
        let mut hosts = vec![];

        for config in &self.config_files {
            for group in &config.groups {
                for c in &group.criteria {
                    if let Criteria::Host(patterns) = c {
                        for pattern in patterns {
                            if pattern.is_literal && !pattern.negated {
                                if !hosts.contains(&pattern.original) {
                                    hosts.push(pattern.original.clone());
                                }
                            }
                        }
                    }
                }
            }
        }

        hosts
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use k9::snapshot;

    #[test]
    fn parse_proxy_command_tokens() {
        let mut config = Config::new();
        config.add_config_string(
            r#"
        Host foo
            ProxyCommand /usr/bin/corp-ssh-helper -dst_username=%r %h %p
            Port 2222
            "#,
        );
        let mut fake_env = ConfigMap::new();
        fake_env.insert("HOME".to_string(), "/home/me".to_string());
        fake_env.insert("USER".to_string(), "me".to_string());
        config.assign_environment(fake_env);

        let opts = config.for_host("foo");
        snapshot!(
            opts,
            r#"
{
    "hostname": "foo",
    "identityfile": "/home/me/.ssh/id_dsa /home/me/.ssh/id_ecdsa /home/me/.ssh/id_ed25519 /home/me/.ssh/id_rsa",
    "port": "2222",
    "proxycommand": "/usr/bin/corp-ssh-helper -dst_username=me foo 2222",
    "user": "me",
    "userknownhostsfile": "/home/me/.ssh/known_hosts /home/me/.ssh/known_hosts2",
}
"#
        );
    }

    #[test]
    fn parse_proxy_command() {
        let mut config = Config::new();
        config.add_config_string(
            r#"
        Host foo
            ProxyCommand /usr/bin/ssh-proxy-helper -oX=Y host 22
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
                MatchGroup {
                    criteria: [
                        Host(
                            [
                                Pattern {
                                    negated: false,
                                    pattern: "^foo$",
                                    original: "foo",
                                    is_literal: true,
                                },
                            ],
                        ),
                    ],
                    context: FirstPass,
                    options: {
                        "proxycommand": "/usr/bin/ssh-proxy-helper -oX=Y host 22",
                    },
                },
            ],
            loaded_files: [],
        },
    ],
    options: {},
    tokens: {},
    environment: None,
}
"#
        );
    }

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
                MatchGroup {
                    criteria: [
                        Host(
                            [
                                Pattern {
                                    negated: false,
                                    pattern: "^foo$",
                                    original: "foo",
                                    is_literal: true,
                                },
                            ],
                        ),
                    ],
                    context: FirstPass,
                    options: {
                        "hostname": "10.0.0.1",
                        "identityfile": "%d/.ssh/id_pub.dsa",
                        "user": "foo",
                    },
                },
            ],
            loaded_files: [],
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
    fn hostname_expansion() {
        let mut config = Config::new();

        let mut fake_env = ConfigMap::new();
        fake_env.insert("HOME".to_string(), "/home/me".to_string());
        fake_env.insert("USER".to_string(), "me".to_string());
        config.assign_environment(fake_env);

        config.add_config_string(
            r#"
        Host foo0 foo1 foo2
            HostName server-%h
            "#,
        );

        let opts = config.for_host("foo0");
        snapshot!(
            opts,
            r#"
{
    "hostname": "server-foo0",
    "identityfile": "/home/me/.ssh/id_dsa /home/me/.ssh/id_ecdsa /home/me/.ssh/id_ed25519 /home/me/.ssh/id_rsa",
    "port": "22",
    "user": "me",
    "userknownhostsfile": "/home/me/.ssh/known_hosts /home/me/.ssh/known_hosts2",
}
"#
        );

        let opts = config.for_host("foo1");
        snapshot!(
            opts,
            r#"
{
    "hostname": "server-foo1",
    "identityfile": "/home/me/.ssh/id_dsa /home/me/.ssh/id_ecdsa /home/me/.ssh/id_ed25519 /home/me/.ssh/id_rsa",
    "port": "22",
    "user": "me",
    "userknownhostsfile": "/home/me/.ssh/known_hosts /home/me/.ssh/known_hosts2",
}
"#
        );

        let opts = config.for_host("foo2");
        snapshot!(
            opts,
            r#"
{
    "hostname": "server-foo2",
    "identityfile": "/home/me/.ssh/id_dsa /home/me/.ssh/id_ecdsa /home/me/.ssh/id_ed25519 /home/me/.ssh/id_rsa",
    "port": "22",
    "user": "me",
    "userknownhostsfile": "/home/me/.ssh/known_hosts /home/me/.ssh/known_hosts2",
}
"#
        );
    }

    #[test]
    fn parse_proxy_command_hostname_expansion() {
        let mut config = Config::new();

        let mut fake_env = ConfigMap::new();
        fake_env.insert("HOME".to_string(), "/home/me".to_string());
        fake_env.insert("USER".to_string(), "me".to_string());
        config.assign_environment(fake_env);

        config.add_config_string(
            r#"
        Host foo
            HostName server-%h
            ProxyCommand nc -x localhost:1080 %h %p
            "#,
        );

        let opts = config.for_host("foo");
        snapshot!(
            opts,
            r#"
{
    "hostname": "server-foo",
    "identityfile": "/home/me/.ssh/id_dsa /home/me/.ssh/id_ecdsa /home/me/.ssh/id_ed25519 /home/me/.ssh/id_rsa",
    "port": "22",
    "proxycommand": "nc -x localhost:1080 server-foo 22",
    "user": "me",
    "userknownhostsfile": "/home/me/.ssh/known_hosts /home/me/.ssh/known_hosts2",
}
"#
        );
    }

    #[test]
    fn multiple_identityfile() {
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
            IdentityFile "~/.ssh/id_pub.rsa"
            "#,
        );

        let opts = config.for_host("foo");
        snapshot!(
            opts,
            r#"
{
    "hostname": "10.0.0.1",
    "identityfile": "/home/me/.ssh/id_pub.dsa /home/me/.ssh/id_pub.rsa",
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
    fn parse_match() {
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
        Match Host 192.168.1.8,wopr
            FowardAgent yes
            IdentityFile "%d/.ssh/id_pub.dsa"

        Match Host !a.b,*.b User fred
            ForwardAgent no
            IdentityAgent "${HOME}/.ssh/agent"

        Match Host !a.b,*.b User me
            ForwardAgent no
            IdentityAgent "${HOME}/.ssh/agent-me"

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
                MatchGroup {
                    criteria: [
                        Host(
                            [
                                Pattern {
                                    negated: false,
                                    pattern: "^192\\.168\\.1\\.8$",
                                    original: "192.168.1.8",
                                    is_literal: true,
                                },
                                Pattern {
                                    negated: false,
                                    pattern: "^wopr$",
                                    original: "wopr",
                                    is_literal: true,
                                },
                            ],
                        ),
                    ],
                    context: FirstPass,
                    options: {
                        "fowardagent": "yes",
                        "identityfile": "%d/.ssh/id_pub.dsa",
                    },
                },
                MatchGroup {
                    criteria: [
                        Host(
                            [
                                Pattern {
                                    negated: true,
                                    pattern: "^a\\.b$",
                                    original: "a.b",
                                    is_literal: true,
                                },
                                Pattern {
                                    negated: false,
                                    pattern: "^.*\\.b$",
                                    original: "*.b",
                                    is_literal: false,
                                },
                            ],
                        ),
                        User(
                            [
                                Pattern {
                                    negated: false,
                                    pattern: "^fred$",
                                    original: "fred",
                                    is_literal: true,
                                },
                            ],
                        ),
                    ],
                    context: FirstPass,
                    options: {
                        "forwardagent": "no",
                        "identityagent": "${HOME}/.ssh/agent",
                    },
                },
                MatchGroup {
                    criteria: [
                        Host(
                            [
                                Pattern {
                                    negated: true,
                                    pattern: "^a\\.b$",
                                    original: "a.b",
                                    is_literal: true,
                                },
                                Pattern {
                                    negated: false,
                                    pattern: "^.*\\.b$",
                                    original: "*.b",
                                    is_literal: false,
                                },
                            ],
                        ),
                        User(
                            [
                                Pattern {
                                    negated: false,
                                    pattern: "^me$",
                                    original: "me",
                                    is_literal: true,
                                },
                            ],
                        ),
                    ],
                    context: FirstPass,
                    options: {
                        "forwardagent": "no",
                        "identityagent": "${HOME}/.ssh/agent-me",
                    },
                },
                MatchGroup {
                    criteria: [
                        Host(
                            [
                                Pattern {
                                    negated: false,
                                    pattern: "^.*$",
                                    original: "*",
                                    is_literal: false,
                                },
                            ],
                        ),
                    ],
                    context: FirstPass,
                    options: {
                        "something": "else",
                    },
                },
            ],
            loaded_files: [],
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

        snapshot!(
            config.enumerate_hosts(),
            r#"
[
    "192.168.1.8",
    "wopr",
]
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
    "identityagent": "/home/me/.ssh/agent-me",
    "identityfile": "/home/me/.ssh/id_dsa /home/me/.ssh/id_ecdsa /home/me/.ssh/id_ed25519 /home/me/.ssh/id_rsa",
    "port": "22",
    "something": "first",
    "user": "me",
    "userknownhostsfile": "/home/me/.ssh/known_hosts /home/me/.ssh/known_hosts2",
}
"#
        );

        let mut fake_env = ConfigMap::new();
        fake_env.insert("HOME".to_string(), "/home/fred".to_string());
        fake_env.insert("USER".to_string(), "fred".to_string());
        config.assign_environment(fake_env);

        let opts = config.for_host("b.b");
        snapshot!(
            opts,
            r#"
{
    "forwardagent": "no",
    "hostname": "b.b",
    "identityagent": "/home/fred/.ssh/agent",
    "identityfile": "/home/fred/.ssh/id_dsa /home/fred/.ssh/id_ecdsa /home/fred/.ssh/id_ed25519 /home/fred/.ssh/id_rsa",
    "port": "22",
    "something": "first",
    "user": "fred",
    "userknownhostsfile": "/home/fred/.ssh/known_hosts /home/fred/.ssh/known_hosts2",
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
        Host 192.168.1.8 wopr
            FowardAgent yes
            IdentityFile "%d/.ssh/id_pub.dsa"

        Host !a.b *.b
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
                MatchGroup {
                    criteria: [
                        Host(
                            [
                                Pattern {
                                    negated: false,
                                    pattern: "^192\\.168\\.1\\.8$",
                                    original: "192.168.1.8",
                                    is_literal: true,
                                },
                                Pattern {
                                    negated: false,
                                    pattern: "^wopr$",
                                    original: "wopr",
                                    is_literal: true,
                                },
                            ],
                        ),
                    ],
                    context: FirstPass,
                    options: {
                        "fowardagent": "yes",
                        "identityfile": "%d/.ssh/id_pub.dsa",
                    },
                },
                MatchGroup {
                    criteria: [
                        Host(
                            [
                                Pattern {
                                    negated: true,
                                    pattern: "^a\\.b$",
                                    original: "a.b",
                                    is_literal: true,
                                },
                                Pattern {
                                    negated: false,
                                    pattern: "^.*\\.b$",
                                    original: "*.b",
                                    is_literal: false,
                                },
                            ],
                        ),
                    ],
                    context: FirstPass,
                    options: {
                        "forwardagent": "no",
                        "identityagent": "${HOME}/.ssh/agent",
                    },
                },
                MatchGroup {
                    criteria: [
                        Host(
                            [
                                Pattern {
                                    negated: false,
                                    pattern: "^.*$",
                                    original: "*",
                                    is_literal: false,
                                },
                            ],
                        ),
                    ],
                    context: FirstPass,
                    options: {
                        "something": "else",
                    },
                },
            ],
            loaded_files: [],
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
