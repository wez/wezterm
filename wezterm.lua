local wezterm = require("wezterm")
local config = wezterm.config_builder()

-- Set Git Bash as the default shell
config.default_prog = { "C:/Program Files/Git/bin/bash.exe", "--login", "-i" }

-- Font and Window Appearance
config.font = wezterm.font_with_fallback({
  "JetBrains Mono",         -- Primary font
  "Nerd Font Symbols",      -- Symbols font for developer icons
  "Noto Color Emoji",       -- Emoji support
})
config.font_size = 15
config.window_decorations = "TITLE | RESIZE"
config.color_scheme = "Dracula"  -- Set a popular dark theme with good color contrast
config.window_background_opacity = 0.9
config.hide_tab_bar_if_only_one_tab = true  -- Hide the tab bar when only one tab is open

-- Color Settings for Inactive Panes
config.inactive_pane_hsb = {
  saturation = 0.9,
  brightness = 0.5,
}

-- Function to split and force specific shells in each split action
local function split_with_specified_shell(direction, shell)
    if direction == "horizontal" then
        return wezterm.action.SplitHorizontal { args = shell }
    else
        return wezterm.action.SplitVertical { args = shell }
    end
end

-- Keybindings
config.keys = {
    -- Close current pane with CTRL+SHIFT+D
    {
        key = 'D',
        mods = 'CTRL|SHIFT',
        action = wezterm.action.CloseCurrentPane { confirm = true },
    },

    -- Git Bash with CTRL+SHIFT+G
    {
        key = 'G',
        mods = 'CTRL|SHIFT',
        action = wezterm.action.SpawnCommandInNewTab {
            args = { "C:/Program Files/Git/bin/bash.exe", "--login", "-i" },
        },
    },

    -- CMD with CTRL+SHIFT+C
    {
        key = 'C',
        mods = 'CTRL|SHIFT',
        action = wezterm.action.SpawnCommandInNewTab {
            args = { "cmd.exe" },
        },
    },

    -- PowerShell with CTRL+SHIFT+P
    {
        key = 'P',
        mods = 'CTRL|SHIFT',
        action = wezterm.action.SpawnCommandInNewTab {
            args = { "pwsh.exe" },
        },
    },

    -- Windows PowerShell with CTRL+SHIFT+W
    {
        key = 'W',
        mods = 'CTRL|SHIFT',
        action = wezterm.action.SpawnCommandInNewTab {
            args = { "powershell.exe" },
        },
    },

    -- WSL Ubuntu with CTRL+SHIFT+U
    {
        key = 'U',
        mods = 'CTRL|SHIFT',
        action = wezterm.action.SpawnCommandInNewTab {
            args = { "wsl.exe", "--distribution", "Ubuntu" },
        },
    },

    -- Zsh under WSL with CTRL+SHIFT+Z
    {
        key = 'Z',
        mods = 'CTRL|SHIFT',
        action = wezterm.action.SpawnCommandInNewTab {
            args = { "wsl.exe", "-e", "zsh" },
        },
    },

    -- Windows PowerShell Splits (These already work)
    -- Vertical split with CTRL+SHIFT+'
    {
        key = '"',
        mods = 'CTRL|SHIFT',
        action = wezterm.action_callback(function(window, pane)
            window:perform_action(
                split_with_specified_shell("vertical", { "powershell.exe" }),
                pane
            )
        end),
    },
    -- Horizontal split with CTRL+SHIFT+|
    {
        key = '|',
        mods = 'CTRL|SHIFT',
        action = wezterm.action_callback(function(window, pane)
            window:perform_action(
                split_with_specified_shell("horizontal", { "powershell.exe" }),
                pane
            )
        end),
    },

    -- CMD Splits
    -- Vertical split with CTRL+SHIFT+M
    {
        key = 'M',
        mods = 'CTRL|SHIFT',
        action = wezterm.action_callback(function(window, pane)
            window:perform_action(
                split_with_specified_shell("vertical", { "cmd.exe" }),
                pane
            )
        end),
    },
    -- Horizontal split with CTRL+SHIFT+N
    {
        key = 'N',
        mods = 'CTRL|SHIFT',
        action = wezterm.action_callback(function(window, pane)
            window:perform_action(
                split_with_specified_shell("horizontal", { "cmd.exe" }),
                pane
            )
        end),
    },

    -- PowerShell Splits
    -- Vertical split with CTRL+SHIFT+S
    {
        key = 'S',
        mods = 'CTRL|SHIFT',
        action = wezterm.action_callback(function(window, pane)
            window:perform_action(
                split_with_specified_shell("vertical", { "pwsh.exe" }),
                pane
            )
        end),
    },
    -- Horizontal split with CTRL+SHIFT+H
    {
        key = 'H',
        mods = 'CTRL|SHIFT',
        action = wezterm.action_callback(function(window, pane)
            window:perform_action(
                split_with_specified_shell("horizontal", { "pwsh.exe" }),
                pane
            )
        end),
    },

    -- Git Bash Splits
    -- Vertical split with CTRL+SHIFT+B
    {
        key = 'B',
        mods = 'CTRL|SHIFT',
        action = wezterm.action_callback(function(window, pane)
            window:perform_action(
                split_with_specified_shell("vertical", { "C:/Program Files/Git/bin/bash.exe", "--login", "-i" }),
                pane
            )
        end),
    },
    -- Horizontal split with CTRL+SHIFT+V
    {
        key = 'V',
        mods = 'CTRL|SHIFT',
        action = wezterm.action_callback(function(window, pane)
            window:perform_action(
                split_with_specified_shell("horizontal", { "C:/Program Files/Git/bin/bash.exe", "--login", "-i" }),
                pane
            )
        end),
    },

    -- Zsh Splits
    -- Vertical split with CTRL+SHIFT+J
    {
        key = 'J',
        mods = 'CTRL|SHIFT',
        action = wezterm.action_callback(function(window, pane)
            window:perform_action(
                split_with_specified_shell("vertical", { "wsl.exe", "-e", "zsh" }),
                pane
            )
        end),
    },
    -- Horizontal split with CTRL+SHIFT+K
    {
        key = 'K',
        mods = 'CTRL|SHIFT',
        action = wezterm.action_callback(function(window, pane)
            window:perform_action(
                split_with_specified_shell("horizontal", { "wsl.exe", "-e", "zsh" }),
                pane
            )
        end),
    },

    -- WSL Ubuntu Splits
    -- Vertical split with CTRL+SHIFT+I
    {
        key = 'I',
        mods = 'CTRL|SHIFT',
        action = wezterm.action_callback(function(window, pane)
            window:perform_action(
                split_with_specified_shell("vertical", { "wsl.exe", "--distribution", "Ubuntu" }),
                pane
            )
        end),
    },
    -- Horizontal split with CTRL+SHIFT+O
    {
        key = 'O',
        mods = 'CTRL|SHIFT',
        action = wezterm.action_callback(function(window, pane)
            window:perform_action(
                split_with_specified_shell("horizontal", { "wsl.exe", "--distribution", "Ubuntu" }),
                pane
            )
        end),
    },
}

-- Custom function to set tab colors based on program with distinctive colors
wezterm.on("format-tab-title", function(tab, tabs, panes, config, hover, max_width)
    local process_name = tab.active_pane.foreground_process_name or ""
    local title = tab.active_pane.title
  
    -- Default background and text colors
    local color = "#505050"      -- Dark gray for unknown/default tabs
    local text_color = "#FFFFFF"  -- White text for contrast
  
    -- Set specific colors and titles based on process
    if process_name:find("pwsh") then
        color = "#4A90E2"    -- Soft blue for PowerShell
        title = "PowerShell"
    elseif process_name:find("cmd") then
        color = "#505050"    -- Darker gray for CMD
        title = "CMD"
    elseif process_name:find("bash") then
        color = "#6B8E23"    -- Olive green for Git Bash
        title = "Git Bash"
    elseif process_name:find("powershell") then
        color = "#8A2BE2"    -- Purple for Windows PowerShell
        title = "Windows PowerShell"
    elseif process_name:find("wsl") or process_name:find("ubuntu") then
        color = "#2E8B57"    -- Sea green for WSL Ubuntu
        title = "WSL (Ubuntu)"
    elseif process_name:find("zsh") then
        color = "#DAA520"    -- Goldenrod for Zsh
        title = "Zsh"
    end
  
    return {
        { Background = { Color = color } },
        { Foreground = { Color = text_color } },
        { Text = " " .. title .. " " },
    }
  end)

-- Custom Status Bar showing the current tab and workspace
wezterm.on("update-right-status", function(window, pane)
  local tab = window:active_tab()
  local index = tab and tab:index() or 0
  window:set_right_status("Tab " .. (index + 1))
end)

return config
