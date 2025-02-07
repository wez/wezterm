use anyhow::{anyhow, Context};
use config::lua::get_or_create_sub_module;
use config::lua::mlua::{self, Lua, Value};
use git2::build::CheckoutBuilder;
use git2::{Remote, Repository};
use luahelper::to_lua;
use std::path::PathBuf;
use tempfile::TempDir;
use wezterm_dynamic::{FromDynamic, ToDynamic};

#[derive(FromDynamic, ToDynamic, Debug)]
struct RepoSpec {
    url: String,
    component: String,
    plugin_dir: PathBuf,
}

/// Given a URL, generate a string that can be used as a directory name.
/// The returned name must be a single valid filesystem component
fn compute_repo_dir(url: &str) -> String {
    let mut dir = String::new();
    for c in url.chars() {
        match c {
            '/' | '\\' => {
                dir.push_str("sZs");
            }
            ':' => {
                dir.push_str("sCs");
            }
            '.' => {
                dir.push_str("sDs");
            }
            '-' | '_' => dir.push(c),
            c if c.is_alphanumeric() => dir.push(c),
            c => dir.push_str(&format!("u{}", c as u32)),
        }
    }
    if dir.ends_with("sZs") {
        dir.truncate(dir.len() - 3);
    }
    dir
}

fn get_remote(repo: &Repository) -> anyhow::Result<Option<Remote>> {
    let remotes = repo.remotes()?;
    for remote in remotes.iter() {
        if let Some(name) = remote {
            let remote = repo.find_remote(name)?;
            return Ok(Some(remote));
        }
    }
    Ok(None)
}

impl RepoSpec {
    fn parse(url: String) -> anyhow::Result<Self> {
        let component = compute_repo_dir(&url);
        if component.starts_with('.') {
            anyhow::bail!("invalid repo spec {url}");
        }

        let plugin_dir = RepoSpec::plugins_dir().join(&component);

        Ok(Self {
            url,
            component,
            plugin_dir,
        })
    }

    fn load_from_dir(path: PathBuf) -> anyhow::Result<Self> {
        let component = path
            .file_name()
            .ok_or_else(|| anyhow!("missing file name!?"))?
            .to_str()
            .ok_or_else(|| anyhow!("{path:?} isn't unicode"))?
            .to_string();

        let plugin_dir = RepoSpec::plugins_dir().join(&component);

        let repo = Repository::open(&path)?;
        let remote = get_remote(&repo)?.ok_or_else(|| anyhow!("no remotes!?"))?;
        let url = remote.url();
        if let Some(url) = url {
            let url = url.to_string();
            return Ok(Self {
                component,
                url,
                plugin_dir,
            });
        }
        anyhow::bail!("Unable to create a complete RepoSpec for repo at {path:?}");
    }

    fn plugins_dir() -> PathBuf {
        config::DATA_DIR.join("plugins")
    }

    fn checkout_path(&self) -> PathBuf {
        Self::plugins_dir().join(&self.component)
    }

    fn is_checked_out(&self) -> bool {
        self.checkout_path().exists()
    }

    fn update(&self) -> anyhow::Result<()> {
        let path = self.checkout_path();
        let repo = Repository::open(&path)?;
        let mut remote = get_remote(&repo)?.ok_or_else(|| anyhow!("no remotes!?"))?;
        remote.connect(git2::Direction::Fetch).context("connect")?;
        let branch = remote
            .default_branch()
            .context("get default branch")?
            .as_str()
            .ok_or_else(|| anyhow!("default branch is not utf8"))?
            .to_string();

        remote.fetch(&[branch], None, None).context("fetch")?;
        let mut merge_info = None;
        repo.fetchhead_foreach(|refname, _remote_url, target_oid, was_merge| {
            if was_merge {
                merge_info.replace((refname.to_string(), *target_oid));
                return true;
            }
            false
        })
        .context("fetchhead_foreach")?;

        let (refname, target_oid) = merge_info.ok_or_else(|| anyhow!("No merge info!?"))?;
        let commit = repo
            .find_annotated_commit(target_oid)
            .context("find_annotated_commit")?;

        let (analysis, _preference) = repo.merge_analysis(&[&commit]).context("merge_analysis")?;
        if analysis.is_up_to_date() {
            log::debug!("{} is up to date!", self.component);
            return Ok(());
        }
        if analysis.is_fast_forward() {
            log::debug!("{} can fast forward!", self.component);
            let mut reference = repo.find_reference(&refname).context("find_reference")?;
            reference
                .set_target(target_oid, "fast forward")
                .context("set_target")?;
            repo.checkout_head(Some(CheckoutBuilder::new().force()))
                .context("checkout_head")?;
            return Ok(());
        }

        log::debug!("{} will merge", self.component);
        repo.merge(&[&commit], None, Some(CheckoutBuilder::new().safe()))
            .context("merge")?;
        Ok(())
    }

    fn check_out(&self) -> anyhow::Result<()> {
        let plugins_dir = Self::plugins_dir();
        std::fs::create_dir_all(&plugins_dir)?;
        let target_dir = TempDir::new_in(&plugins_dir)?;
        log::debug!("Cloning {} into temporary dir {target_dir:?}", self.url);
        Repository::clone_recurse(&self.url, target_dir.path())?;
        let target_dir = target_dir.into_path();
        let checkout_path = self.checkout_path();
        match std::fs::rename(&target_dir, &checkout_path) {
            Ok(_) => {
                log::info!("Cloned {} into {checkout_path:?}", self.url);
                Ok(())
            }
            Err(err) => {
                log::error!(
                    "Failed to rename {target_dir:?} -> {:?}, removing temporary dir",
                    self.checkout_path()
                );
                if let Err(err) = std::fs::remove_dir_all(&target_dir) {
                    log::error!(
                        "Failed to remove {target_dir:?}: {err:#}, \
                         you will need to remove it manually"
                    );
                }
                Err(err.into())
            }
        }
    }
}

fn require_plugin(lua: &Lua, url: String) -> anyhow::Result<Value> {
    let spec = RepoSpec::parse(url)?;

    if !spec.is_checked_out() {
        spec.check_out()?;
    }

    let require: mlua::Function = lua.globals().get("require")?;
    match require.call::<_, Value>(spec.component.to_string()) {
        Ok(value) => Ok(value),
        Err(err) => {
            log::error!(
                "Failed to require {} which is stored in {:?}: {err:#}",
                spec.component,
                spec.checkout_path()
            );
            Err(err.into())
        }
    }
}

fn list_plugins() -> anyhow::Result<Vec<RepoSpec>> {
    let mut plugins = vec![];

    let plugins_dir = RepoSpec::plugins_dir();
    std::fs::create_dir_all(&plugins_dir)?;

    for entry in plugins_dir.read_dir()? {
        let entry = entry?;
        if entry.path().is_dir() {
            plugins.push(RepoSpec::load_from_dir(entry.path())?);
        }
    }

    Ok(plugins)
}

pub fn register(lua: &Lua) -> anyhow::Result<()> {
    let plugin_mod = get_or_create_sub_module(lua, "plugin")?;
    plugin_mod.set(
        "require",
        lua.create_function(|lua: &Lua, repo_spec: String| {
            require_plugin(lua, repo_spec).map_err(|e| mlua::Error::external(format!("{e:#}")))
        })?,
    )?;

    plugin_mod.set(
        "list",
        lua.create_function(|lua, _: ()| {
            let plugins = list_plugins().map_err(|e| mlua::Error::external(format!("{e:#}")))?;
            to_lua(lua, plugins)
        })?,
    )?;

    plugin_mod.set(
        "update_all",
        lua.create_function(|_, _: ()| {
            let plugins = list_plugins().map_err(|e| mlua::Error::external(format!("{e:#}")))?;
            for p in plugins {
                match p.update() {
                    Ok(_) => log::info!("Updated {p:?}"),
                    Err(err) => log::error!("Failed to update {p:?}: {err:#}"),
                }
            }
            Ok(())
        })?,
    )?;
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_compute_repo_dir() {
        for (input, expect) in &[
            ("foo", "foo"),
            (
                "githubsDscom/wezterm/wezterm-plugins",
                "githubsDscomsZsweztermsZswezterm-plugins",
            ),
            ("localhost:8080/repo", "localhostsCs8080sZsrepo"),
        ] {
            let result = compute_repo_dir(input);
            assert_eq!(&result, expect, "for input {input}");
        }
    }
}
