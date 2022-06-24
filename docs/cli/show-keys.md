# `wezterm show-keys`

*Since: nightly builds only*


Prints the complete set of key assignments based on your config file.

The command shows each key table as well as the set of mouse bindings.

A truncated example of the output is shown below.


```
Default key table
-----------------

        CTRL                 Tab                ->   ActivateTabRelative(1)
        SHIFT | CTRL         Tab                ->   ActivateTabRelative(-1)
        ...

Key Table: copy_mode
--------------------

                Tab          ->   CopyMode(MoveForwardWord)
        SHIFT   Tab          ->   CopyMode(MoveBackwardWord)
        SHIFT   $            ->   CopyMode(MoveToEndOfLineContent)
        ...

Key Table: search_mode
----------------------

               Enter       ->   CopyMode(PriorMatch)
               Escape      ->   CopyMode(Close)
        CTRL   n           ->   CopyMode(NextMatch)
        ...

Mouse
-----

                       Down { streak: 1, button: Left }     ->   SelectTextAtMouseCursor(Cell)
        SHIFT          Down { streak: 1, button: Left }     ->   ExtendSelectionToMouseCursor(None)
        ALT            Down { streak: 1, button: Left }     ->   SelectTextAtMouseCursor(Block)
        ...
```
