# https://mkdocs-macros-plugin.readthedocs.io/en/latest/macros/
def define_env(env):
    @env.macro
    def since(vers, inline=False):
        if vers == "nightly" and not inline:
            return """
???+ info "*Since: Nightly Builds Only*"
    The feature described in this section requires a nightly build of wezterm.
    You can obtain a nightly build by following the instructions from the
    [Download](/wezterm/installation.html) section.
"""

        if vers == "nightly" and inline:
            return """
!!! info "*Since: Nightly Builds Only*"
    *The feature described in this section requires a nightly build of wezterm.
    You can obtain a nightly build by following the instructions from the
    [Download](/wezterm/installation.html) section.*
"""

        if inline:
            return f"""
!!! info "*Since: Version {vers}*"
    *The feature described in this section requires version {vers} of wezterm,
    or a more recent version.*
"""

        return f"""
??? info "*Since: Version {vers}*"
    The feature described in this section requires version {vers} of wezterm,
    or a more recent version.
"""
