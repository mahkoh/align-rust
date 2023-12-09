align
=====

Align text.

Example
-------

Before:

	static const char *sd_cmd_arg_str[] = {
		[SD_CMD_CLEAR_LIBRARY] = "clear -l",
		[SD_CMD_CLEAR_PLAYLIST] = "clear -p",
		[SD_CMD_CLEAR_QUEUE] = "clear -q",
		[SD_CMD_LOAD] = "load %s",
		[SD_CMD_NEXT] = "player-next",
	};

After:

	static const char *sd_cmd_arg_str[] = {
		[SD_CMD_CLEAR_LIBRARY]  = "clear -l",
		[SD_CMD_CLEAR_PLAYLIST] = "clear -p",
		[SD_CMD_CLEAR_QUEUE]    = "clear -q",
		[SD_CMD_LOAD]           = "load %s",
		[SD_CMD_NEXT]           = "player-next",
	};

Before:

    int a = 111; // a
    int aa = 11; // aa
    int aaa = 1; // aaa

After `align "<><"`:

    int   a = 111; // a
    int  aa = 11;  // aa
    int aaa = 1;   // aaa

Note that the last alignment specifier, `<`, is used for all subsequent columns.

Vim
---

    vnoremap <leader>c :!align<cr>


`column -t`
-----------

This program differs from `column -t` in the following ways:

- Empty lines aren't deleted.
- The text keeps its indentation.
- You can align right.
- Unicode support.

License
-------

GPL 3
