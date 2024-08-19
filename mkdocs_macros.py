import json

# https://mkdocs-macros-plugin.readthedocs.io/en/latest/macros/
def define_env(env):
    with open("docs/releases.json") as f:
        for (k, v) in json.load(f).items():
            env.variables[k] = v


    @env.macro
    def since(vers, outline=False, inline=False):
        if vers == "nightly":
            # Determine the relative path traversal to the root,
            # so that we can emit the link to the install page
            rel_root = "../" * (len(env.page.url.split('/')) - 1)
            first_line = "*Since: Nightly Builds Only*"
            expanded = "+"
            blurb = f"""
    The functionality described in this section requires a nightly build of wezterm.
    You can obtain a nightly build by following the instructions from the
    [Download]({rel_root}installation.md) section.
"""
        else:
            first_line = f"*Since: Version {vers}*"
            expanded = ""
            blurb = f"""
    *The functionality described in this section requires version {vers} of wezterm,
    or a more recent version.*
"""

        if outline:
            return f"""
!!! info "{first_line}"
{blurb}
"""

        if inline:
            return f"({first_line})"

        return f"""
???{expanded} info "{first_line}"
{blurb}
"""
