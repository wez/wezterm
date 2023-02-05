# shellcheck shell=bash

# This file hooks up shell integration for wezterm.
# It is suitable for zsh and bash.
#
# Although wezterm is mentioned here, the sequences used are not wezterm
# specific and may provide the same functionality for other terminals.  Most
# terminals are good at ignoring OSC sequences that they don't understand, but
# if not there are some bypasses:
#
# WEZTERM_SHELL_SKIP_ALL - disables all
# WEZTERM_SHELL_SKIP_SEMANTIC_ZONES - disables zones
# WEZTERM_SHELL_SKIP_CWD - disables OSC 7 cwd setting
# WEZTERM_SHELL_SKIP_USER_VARS - disable user vars that capture information
#                                about running programs

# shellcheck disable=SC2166
if [ -z "${BASH_VERSION}" -a -z "${ZSH_NAME}" ] ; then
  # Only for bash or zsh
  return 0
fi

if [ "${WEZTERM_SHELL_SKIP_ALL}" = "1" ] ; then
  return 0
fi

if [[ $- != *i* ]] ; then
  # Shell integration is only useful in interactive sessions
  return 0
fi

case "$TERM" in
  linux | dumb )
    # Avoid terminals that don't like OSC sequences
    return 0
  ;;
esac

# This function wraps bash-preexec.sh so that it can be included verbatim
# in this file, even though it uses `return` to short-circuit in some cases.
__wezterm_install_bash_prexec() {

# bash-preexec.sh -- Bash support for ZSH-like 'preexec' and 'precmd' functions.
# https://github.com/rcaloras/bash-preexec
#
# 'preexec' functions are executed before each interactive command is
# executed, with the interactive command as its argument. The 'precmd'
# function is executed before each prompt is displayed.
#
# Author: Ryan Caloras (ryan@bashhub.com)
# Forked from Original Author: Glyph Lefkowitz
#
# V0.4.1
#

# General Usage:
#
#  1. Source this file at the end of your bash profile so as not to interfere
#     with anything else that's using PROMPT_COMMAND.
#
#  2. Add any precmd or preexec functions by appending them to their arrays:
#       e.g.
#       precmd_functions+=(my_precmd_function)
#       precmd_functions+=(some_other_precmd_function)
#
#       preexec_functions+=(my_preexec_function)
#
#  3. Consider changing anything using the DEBUG trap or PROMPT_COMMAND
#     to use preexec and precmd instead. Preexisting usages will be
#     preserved, but doing so manually may be less surprising.
#
#  Note: This module requires two Bash features which you must not otherwise be
#  using: the "DEBUG" trap, and the "PROMPT_COMMAND" variable. If you override
#  either of these after bash-preexec has been installed it will most likely break.

# Make sure this is bash that's running and return otherwise.
if [[ -z "${BASH_VERSION:-}" ]]; then
    return 1;
fi

# Avoid duplicate inclusion
if [[ "${__bp_imported:-}" == "defined" ]]; then
    return 0
fi
__bp_imported="defined"

# Should be available to each precmd and preexec
# functions, should they want it. $? and $_ are available as $? and $_, but
# $PIPESTATUS is available only in a copy, $BP_PIPESTATUS.
# TODO: Figure out how to restore PIPESTATUS before each precmd or preexec
# function.
__bp_last_ret_value="$?"
BP_PIPESTATUS=("${PIPESTATUS[@]}")
__bp_last_argument_prev_command="$_"

__bp_inside_precmd=0
__bp_inside_preexec=0

# Initial PROMPT_COMMAND string that is removed from PROMPT_COMMAND post __bp_install
__bp_install_string=$'__bp_trap_string="$(trap -p DEBUG)"\ntrap - DEBUG\n__bp_install'

# Fails if any of the given variables are readonly
# Reference https://stackoverflow.com/a/4441178
__bp_require_not_readonly() {
  local var
  for var; do
    if ! ( unset "$var" 2> /dev/null ); then
      echo "bash-preexec requires write access to ${var}" >&2
      return 1
    fi
  done
}

# Remove ignorespace and or replace ignoreboth from HISTCONTROL
# so we can accurately invoke preexec with a command from our
# history even if it starts with a space.
__bp_adjust_histcontrol() {
    local histcontrol
    histcontrol="${HISTCONTROL//ignorespace}"
    # Replace ignoreboth with ignoredups
    if [[ "$histcontrol" == *"ignoreboth"* ]]; then
        histcontrol="ignoredups:${histcontrol//ignoreboth}"
    fi;
    export HISTCONTROL="$histcontrol"
}

# This variable describes whether we are currently in "interactive mode";
# i.e. whether this shell has just executed a prompt and is waiting for user
# input.  It documents whether the current command invoked by the trace hook is
# run interactively by the user; it's set immediately after the prompt hook,
# and unset as soon as the trace hook is run.
__bp_preexec_interactive_mode=""

# Trims leading and trailing whitespace from $2 and writes it to the variable
# name passed as $1
__bp_trim_whitespace() {
    local var=${1:?} text=${2:-}
    text="${text#"${text%%[![:space:]]*}"}"   # remove leading whitespace characters
    text="${text%"${text##*[![:space:]]}"}"   # remove trailing whitespace characters
    printf -v "$var" '%s' "$text"
}


# Trims whitespace and removes any leading or trailing semicolons from $2 and
# writes the resulting string to the variable name passed as $1. Used for
# manipulating substrings in PROMPT_COMMAND
__bp_sanitize_string() {
    local var=${1:?} text=${2:-} sanitized
    __bp_trim_whitespace sanitized "$text"
    sanitized=${sanitized%;}
    sanitized=${sanitized#;}
    __bp_trim_whitespace sanitized "$sanitized"
    printf -v "$var" '%s' "$sanitized"
}

# This function is installed as part of the PROMPT_COMMAND;
# It sets a variable to indicate that the prompt was just displayed,
# to allow the DEBUG trap to know that the next command is likely interactive.
__bp_interactive_mode() {
    __bp_preexec_interactive_mode="on";
}


# This function is installed as part of the PROMPT_COMMAND.
# It will invoke any functions defined in the precmd_functions array.
__bp_precmd_invoke_cmd() {
    # Save the returned value from our last command, and from each process in
    # its pipeline. Note: this MUST be the first thing done in this function.
    __bp_last_ret_value="$?" BP_PIPESTATUS=("${PIPESTATUS[@]}")

    # Don't invoke precmds if we are inside an execution of an "original
    # prompt command" by another precmd execution loop. This avoids infinite
    # recursion.
    if (( __bp_inside_precmd > 0 )); then
      return
    fi
    local __bp_inside_precmd=1

    # Invoke every function defined in our function array.
    local precmd_function
    for precmd_function in "${precmd_functions[@]}"; do

        # Only execute this function if it actually exists.
        # Test existence of functions with: declare -[Ff]
        if type -t "$precmd_function" 1>/dev/null; then
            __bp_set_ret_value "$__bp_last_ret_value" "$__bp_last_argument_prev_command"
            # Quote our function invocation to prevent issues with IFS
            "$precmd_function"
        fi
    done

    return $__bp_last_ret_value
}

# Sets a return value in $?. We may want to get access to the $? variable in our
# precmd functions. This is available for instance in zsh. We can simulate it in bash
# by setting the value here.
__bp_set_ret_value() {
    return ${1:-}
}

__bp_in_prompt_command() {

    local prompt_command_array
    IFS=$'\n;' read -rd '' -a prompt_command_array <<< "$PROMPT_COMMAND"

    local trimmed_arg
    __bp_trim_whitespace trimmed_arg "${1:-}"

    local command trimmed_command
    for command in "${prompt_command_array[@]:-}"; do
        __bp_trim_whitespace trimmed_command "$command"
        if [[ "$trimmed_command" == "$trimmed_arg" ]]; then
            return 0
        fi
    done

    return 1
}

# This function is installed as the DEBUG trap.  It is invoked before each
# interactive prompt display.  Its purpose is to inspect the current
# environment to attempt to detect if the current command is being invoked
# interactively, and invoke 'preexec' if so.
__bp_preexec_invoke_exec() {

    # Save the contents of $_ so that it can be restored later on.
    # https://stackoverflow.com/questions/40944532/bash-preserve-in-a-debug-trap#40944702
    __bp_last_argument_prev_command="${1:-}"
    # Don't invoke preexecs if we are inside of another preexec.
    if (( __bp_inside_preexec > 0 )); then
      return
    fi
    local __bp_inside_preexec=1

    # Checks if the file descriptor is not standard out (i.e. '1')
    # __bp_delay_install checks if we're in test. Needed for bats to run.
    # Prevents preexec from being invoked for functions in PS1
    if [[ ! -t 1 && -z "${__bp_delay_install:-}" ]]; then
        return
    fi

    if [[ -n "${COMP_LINE:-}" ]]; then
        # We're in the middle of a completer. This obviously can't be
        # an interactively issued command.
        return
    fi
    if [[ -z "${__bp_preexec_interactive_mode:-}" ]]; then
        # We're doing something related to displaying the prompt.  Let the
        # prompt set the title instead of me.
        return
    else
        # If we're in a subshell, then the prompt won't be re-displayed to put
        # us back into interactive mode, so let's not set the variable back.
        # In other words, if you have a subshell like
        #   (sleep 1; sleep 2)
        # You want to see the 'sleep 2' as a set_command_title as well.
        if [[ 0 -eq "${BASH_SUBSHELL:-}" ]]; then
            __bp_preexec_interactive_mode=""
        fi
    fi

    if  __bp_in_prompt_command "${BASH_COMMAND:-}"; then
        # If we're executing something inside our prompt_command then we don't
        # want to call preexec. Bash prior to 3.1 can't detect this at all :/
        __bp_preexec_interactive_mode=""
        return
    fi

    local this_command
    this_command=$(
        export LC_ALL=C
        HISTTIMEFORMAT= builtin history 1 | sed '1 s/^ *[0-9][0-9]*[* ] //'
    )

    # Sanity check to make sure we have something to invoke our function with.
    if [[ -z "$this_command" ]]; then
        return
    fi

    # Invoke every function defined in our function array.
    local preexec_function
    local preexec_function_ret_value
    local preexec_ret_value=0
    for preexec_function in "${preexec_functions[@]:-}"; do

        # Only execute each function if it actually exists.
        # Test existence of function with: declare -[fF]
        if type -t "$preexec_function" 1>/dev/null; then
            __bp_set_ret_value ${__bp_last_ret_value:-}
            # Quote our function invocation to prevent issues with IFS
            "$preexec_function" "$this_command"
            preexec_function_ret_value="$?"
            if [[ "$preexec_function_ret_value" != 0 ]]; then
                preexec_ret_value="$preexec_function_ret_value"
            fi
        fi
    done

    # Restore the last argument of the last executed command, and set the return
    # value of the DEBUG trap to be the return code of the last preexec function
    # to return an error.
    # If `extdebug` is enabled a non-zero return value from any preexec function
    # will cause the user's command not to execute.
    # Run `shopt -s extdebug` to enable
    __bp_set_ret_value "$preexec_ret_value" "$__bp_last_argument_prev_command"
}

__bp_install() {
    # Exit if we already have this installed.
    if [[ "${PROMPT_COMMAND:-}" == *"__bp_precmd_invoke_cmd"* ]]; then
        return 1;
    fi

    trap '__bp_preexec_invoke_exec "$_"' DEBUG

    # Preserve any prior DEBUG trap as a preexec function
    local prior_trap=$(sed "s/[^']*'\(.*\)'[^']*/\1/" <<<"${__bp_trap_string:-}")
    unset __bp_trap_string
    if [[ -n "$prior_trap" ]]; then
        eval '__bp_original_debug_trap() {
          '"$prior_trap"'
        }'
        preexec_functions+=(__bp_original_debug_trap)
    fi

    # Adjust our HISTCONTROL Variable if needed.
    __bp_adjust_histcontrol

    # Issue #25. Setting debug trap for subshells causes sessions to exit for
    # backgrounded subshell commands (e.g. (pwd)& ). Believe this is a bug in Bash.
    #
    # Disabling this by default. It can be enabled by setting this variable.
    if [[ -n "${__bp_enable_subshells:-}" ]]; then

        # Set so debug trap will work be invoked in subshells.
        set -o functrace > /dev/null 2>&1
        shopt -s extdebug > /dev/null 2>&1
    fi;

    local existing_prompt_command
    # Remove setting our trap install string and sanitize the existing prompt command string
    existing_prompt_command="${PROMPT_COMMAND//$__bp_install_string[;$'\n']}" # Edge case of appending to PROMPT_COMMAND
    existing_prompt_command="${existing_prompt_command//$__bp_install_string}"
    __bp_sanitize_string existing_prompt_command "$existing_prompt_command"

    # Install our hooks in PROMPT_COMMAND to allow our trap to know when we've
    # actually entered something.
    PROMPT_COMMAND=$'__bp_precmd_invoke_cmd\n'
    if [[ -n "$existing_prompt_command" ]]; then
        PROMPT_COMMAND+=${existing_prompt_command}$'\n'
    fi;
    PROMPT_COMMAND+='__bp_interactive_mode'

    # Add two functions to our arrays for convenience
    # of definition.
    precmd_functions+=(precmd)
    preexec_functions+=(preexec)

    # Invoke our two functions manually that were added to $PROMPT_COMMAND
    __bp_precmd_invoke_cmd
    __bp_interactive_mode
}

# Sets an installation string as part of our PROMPT_COMMAND to install
# after our session has started. This allows bash-preexec to be included
# at any point in our bash profile.
__bp_install_after_session_init() {
    # bash-preexec needs to modify these variables in order to work correctly
    # if it can't, just stop the installation
    __bp_require_not_readonly PROMPT_COMMAND HISTCONTROL HISTTIMEFORMAT || return

    local sanitized_prompt_command
    __bp_sanitize_string sanitized_prompt_command "$PROMPT_COMMAND"
    if [[ -n "$sanitized_prompt_command" ]]; then
        PROMPT_COMMAND=${sanitized_prompt_command}$'\n'
    fi;
    PROMPT_COMMAND+=${__bp_install_string}
}

# Run our install so long as we're not delaying it.
if [[ -z "${__bp_delay_install:-}" ]]; then
    __bp_install_after_session_init
fi;

} # end of __wezterm_install_bash_prexec

# blesh provides it's own preexec mechanism which is recommended over bash-preexec
# See https://github.com/akinomyoga/ble.sh/wiki/Manual-%C2%A71-Introduction#user-content-fn-blehook for more details
if [[ ! -n "$BLE_VERSION" ]]; then
  __wezterm_install_bash_prexec
fi

# This function emits an OSC 1337 sequence to set a user var
# associated with the current terminal pane.
# It requires the `base64` utility to be available in the path.
__wezterm_set_user_var() {
  if hash base64 2>/dev/null ; then
    if [[ -z "${TMUX}" ]] ; then
      printf "\033]1337;SetUserVar=%s=%s\007" "$1" `echo -n "$2" | base64`
    else
      # <https://github.com/tmux/tmux/wiki/FAQ#what-is-the-passthrough-escape-sequence-and-how-do-i-use-it>
      # Note that you ALSO need to add "set -g allow-passthrough on" to your tmux.conf
      printf "\033Ptmux;\033\033]1337;SetUserVar=%s=%s\007\033\\" "$1" `echo -n "$2" | base64`
    fi
  fi
}

# This function emits an OSC 7 sequence to inform the terminal
# of the current working directory.  It prefers to use a helper
# command provided by wezterm if wezterm is installed, but falls
# back to a simple printf command otherwise.
__wezterm_osc7() {
  if hash wezterm 2>/dev/null ; then
    wezterm set-working-directory 2>/dev/null && return 0
    # If the command failed (perhaps the installed wezterm
    # is too old?) then fall back to the simple version below.
  fi
  printf "\033]7;file://%s%s\033\\" "${HOSTNAME}" "${PWD}"
}

# The semantic precmd and prexec functions generate semantic
# zones, marking up the prompt, the user input and the command
# output so that the terminal can better reason about the display.
__wezterm_semantic_precmd_executing=""
__wezterm_semantic_precmd() {
  local ret="$?"
  if [[ "$__wezterm_semantic_precmd_executing" != "0" ]] ; then
    __wezterm_save_ps1="$PS1"
    __wezterm_save_ps2="$PS2"
    # Markup the left and right prompts so that the terminal
    # knows that they are semantically prompt output.
    if [[ -n "$ZSH_NAME" ]] ; then
      PS1=$'%{\e]133;P;k=i\a%}'$PS1$'%{\e]133;B\a%}'
      PS2=$'%{\e]133;P;k=s\a%}'$PS2$'%{\e]133;B\a%}'
    else
      PS1='\[\e]133;P;k=i\a\]'$PS1'\[\e]133;B\a\]'
      PS2='\[\e]133;P;k=s\a\]'$PS2'\[\e]133;B\a\]'
    fi
  fi
  if [[ "$__wezterm_semantic_precmd_executing" != "" ]] ; then
    # Report last command status
    printf "\033]133;D;%s;aid=%s\007" "$ret" "$$"
  fi
  # Fresh line and start the prompt
  printf "\033]133;A;cl=m;aid=%s\007" "$$"
  __wezterm_semantic_precmd_executing=0
}

function __wezterm_semantic_preexec() {
  # Restore the original PS1/PS2
  PS1="$__wezterm_save_ps1"
  PS2="$__wezterm_save_ps2"
  # Indicate that the command output begins here
  printf "\033]133;C;\007"
  __wezterm_semantic_precmd_executing=1
}

__wezterm_user_vars_precmd() {
  __wezterm_set_user_var "WEZTERM_PROG" ""
  __wezterm_set_user_var "WEZTERM_USER" "$(id -un)"

  # Indicate whether this pane is running inside tmux or not
  if [[ -n "${TMUX}" ]] ; then
    __wezterm_set_user_var "WEZTERM_IN_TMUX" "1"
  else
    __wezterm_set_user_var "WEZTERM_IN_TMUX" "0"
  fi

  # You may set WEZTERM_HOSTNAME to a name you want to use instead
  # of calling out to the hostname executable on every prompt print.
  if [[ -z "${WEZTERM_HOSTNAME}" ]] ; then
    if hash hostname 2>/dev/null ; then
      __wezterm_set_user_var "WEZTERM_HOST" "$(hostname)"
    elif hash hostnamectl 2>/dev/null ; then
      __wezterm_set_user_var "WEZTERM_HOST" "$(hostnamectl hostname)"
    fi
  else
    __wezterm_set_user_var "WEZTERM_HOST" "${WEZTERM_HOSTNAME}"
  fi
}

__wezterm_user_vars_preexec() {
  # Tell wezterm the full command that is being run
  __wezterm_set_user_var "WEZTERM_PROG" "$1"
}

# Register the various functions; take care to perform osc7 after
# the semantic zones as we don't want to perturb the last command
# status before we've had a chance to report it to the terminal
if [[ -z "${WEZTERM_SHELL_SKIP_SEMANTIC_ZONES}" ]]; then
  if [[ -n "$BLE_VERSION" ]]; then
    blehook PRECMD+=__wezterm_semantic_precmd
    blehook PREEXEC+=__wezterm_semantic_preexec
  else
    precmd_functions+=(__wezterm_semantic_precmd)
    preexec_functions+=(__wezterm_semantic_preexec)
  fi
fi

if [[ -z "${WEZTERM_SHELL_SKIP_USER_VARS}" ]]; then
  if [[ -n "$BLE_VERSION" ]]; then
    blehook PRECMD+=__wezterm_user_vars_precmd
    blehook PREEXEC+=__wezterm_user_vars_preexec
  else
    precmd_functions+=(__wezterm_user_vars_precmd)
    preexec_functions+=(__wezterm_user_vars_preexec)
  fi
fi

if [[ -z "${WEZTERM_SHELL_SKIP_CWD}" ]] ; then
  if [[ -n "$BLE_VERSION" ]]; then
    blehook PRECMD+=__wezterm_osc7
  else
    precmd_functions+=(__wezterm_osc7)
  fi
fi

true
