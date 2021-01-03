# `wezterm.on(event_name, callback)`

*Since: 20201031-154415-9614e117*

This function follows the html/javascript naming for defining event handlers.

`wezterm.on` causes your specified `callback` to be called when `event_name`
is emitted.  Events can be emitted by wezterm itself, or through code/configuration
that you specify.

`wezterm.on` can register multiple callbacks for the same event; internally
an ordered list of callbacks is maintained for each event.  When the event
is emitted, each of the registered callbacks is called in the order that
they were registered.

If a callback returns `false` it will prevent any callbacks that were registered
after it from being triggered for the current event.  Some events have
a defined default action; returning `false` will prevent that default action
from being taken for the current event.

There is no way to de-register an event handler.  However, since the Lua
state is built from scratch when the configuration is reloaded, simply
reloading the configuration will clear any existing event handlers.

## Predefined Events

### `open-uri`

The `open-uri` event is emitted when the `CompleteSelectionOrOpenLinkAtMouseCursor`
key/mouse assignment is triggered.

The default action is to open the active URI in your browser, but if you
register for this event you can co-opt the default behavior.

For example, if you prefer to launch your preferred MUA in a new window
in response to clicking on `mailto:` URLs, you could do something like:

```lua
local wezterm = require 'wezterm';

wezterm.on("open-uri", function(window, pane, uri)
  local start, end = uri:find("mailto:")
  if start == 1 then
    local recipient = uri:sub(end+1)
    window.perform_action(wezterm.action{SpawnCommandInNewWindow={
         args={"mutt", recipient}
      }});
    -- prevent the default action from opening in a browser
    return false
  end
  -- otherwise, by not specifying a return value, we allow later
  -- handlers and ultimately the default action to caused the
  -- URI to be opened in the browser
end)
```

The first event parameter is a [`window` object](../window/index.md) that
represents the gui window.

The second event parameter is a [`pane` object](../pane/index.md) that
represents the pane.

The third event parameter is the URI string.

### `open-file`

The `open-file` event is emitted when the `CompleteSelectionOrOpenLinkAtMouseCursor`
key/mouse assignment is triggered.

The default action is to open the active URI in your browser, but if you
register for this event you can co-opt the default behavior.

For example, if you prefer to launch your preferred MUA in a new window
in response to clicking on `mailto:` URLs, you could do something like:

```lua
local wezterm = require 'wezterm';
local function isEmpty(s)
  -- Sample use
  -- if isempty(foo) then
  --    foo = "default value"
  -- end
  return s == nil or s == ''
end
local function runAction(window, pane, name, lineno)
  -- Sample use
  -- runAction(window, pane, name, lineno)
  local action = wezterm.action{SpawnCommandInNewWindow={
    -- args={"vim", "+"..lineno, name}
    -- args={"nano", "+"..lineno, name}
    -- args={"pstorm", name..":"..lineno}
    -- args={"mine", name..":"..lineno}
    -- args={"webstorm", name..":"..lineno}
    -- args={"charm", name..":"..lineno}
    -- args={"subl", name..":"..lineno}
    -- args={"brackets", name}
    -- args={"code", "-g", name..":"..lineno}
    args={"code-insiders", "-g", name..":"..lineno}
  }};
  window:perform_action(action, pane);
end

wezterm.on("open-file", function(window, pane, uri)
  local m_fr, m_to = uri:find("hyperfile:");
  if m_fr == 1 then
    local name = uri:sub(m_to+1);
    local m_fr = name:find(":");
    if isEmpty(m_fr) then
      local m_fr = 0
      local m_to = 0
      local m_fr = name:find("Diff in");
      if false == isEmpty(m_fr) then
        local m_fr, m_to = name:find("Diff in ");
        local name = name:sub(m_to+1);
        local m_fr = name:find(" at line ");
        if false == isEmpty(m_fr) then
          local m_fr, m_to = name:find(" at line ");
          local lineno = name:sub(m_to+1)
          name = name:sub(1, m_fr-1)
          runAction(window, pane, name, lineno);
        end
      end
    else
      local m_fr, m_to = name:find(":");
      local lineno = name:sub(m_to+1);
      name = name:sub(1, m_to-1)
      runAction(window, pane, name, lineno);
    end
    -- prevent the default action from opening in a browser
    return false
  end
  -- otherwise, by not specifying a return value, we allow later
  -- handlers and ultimately the default action to caused the
  -- URI to be opened in the browser
end)
return {
  hyperlink_rules = {
    {
      regex = "^\\s*[a-zA-Z0-9/_\\-\\. ]+\\.?[a-zA-Z0-9]+:[0-9]+",
      format = "hyperfile:$0"
    },
    {
       regex = "Diff in [a-zA-Z0-9/_\\-\\. ]+\\.?[a-zA-Z0-9]+",
       format = "hyperfile:$1:$2"
    }
  }
}
```

The first event parameter is a [`window` object](../window/index.md) that
represents the gui window.

The second event parameter is a [`pane` object](../pane/index.md) that
represents the pane.

The third event parameter is the "FILEPATH with a LINE NUMBER" string.

## Custom Events

You may register handlers for arbitrary events for which wezterm itself
has no special knowledge.  It is recommended that you avoid event names
that are likely to be used future versions of wezterm in order to avoid
unexpected behavior if/when those names might be used in future.

The `wezterm.emit` function and the `EmitEvent` key assignment can be used
emit events.

In this example, a key is assigned to capture the content of the active
pane, write it to a file and then open that file in the `vim` editor:

```lua
local wezterm = require 'wezterm';
local io = require 'io';
local os = require 'os';

wezterm.on("trigger-vim-with-scrollback", function(window, pane)
  -- Retrieve the current viewport's text.
  -- Pass an optional number of lines (eg: 2000) to retrieve
  -- that number of lines starting from the bottom of the viewport.
  local scrollback = pane:get_lines_as_text();

  -- Create a temporary file to pass to vim
  local name = os.tmpname();
  local f = io.open(name, "w+");
  f:write(scrollback);
  f:flush();
  f:close();

  -- Open a new window running vim and tell it to open the file
  window:perform_action(wezterm.action{SpawnCommandInNewWindow={
    args={"vim", name}}
  }, pane)

  -- wait "enough" time for vim to read the file before we remove it.
  -- The window creation and process spawn are asynchronous
  -- wrt. running this script and are not awaitable, so we just pick
  -- a number.  We don't strictly need to remove this file, but it
  -- is nice to avoid cluttering up the temporary file directory
  -- location.
  wezterm.sleep_ms(1000);
  os.remove(name);
end)

return {
  keys = {
    {key="E", mods="CTRL",
      action=wezterm.action{EmitEvent="trigger-vim-with-scrollback"}},
  }
}
```

The first event parameter is a [`window` object](../window/index.md) that
represents the gui window.

The second event parameter is a [`pane` object](../pane/index.md) that
represents the pane.
