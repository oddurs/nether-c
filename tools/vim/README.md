# `tools/vim/nether.vim`

Thin Vim/Neovim adapter for [`nether lamp <cairn> --at <byte>`](../../spec/08-rites.md#841-inspecting-a-byte-offset-in-an-editor).
There is no language server, no background daemon, and nothing running behind
your cursor — Vim shells out to `nether lamp` and shows what the ledger already
knows.

## Installation

For a single session, source the script directly:

```vim
:source tools/vim/nether.vim
```

For permanent use, place or symlink `tools/vim/nether.vim` into a Vim package
directory (for example `~/.vim/pack/nether/start/nether/plugin/nether.vim`) or
into Neovim's plugin directory (`~/.config/nvim/plugin/nether.vim`).

## Configuration

- `g:nether_bin` — path or command name for the `nether` executable (defaults
  to `'nether'`).
- `NETHER_STORE` — environment variable honoured by `nether lamp` when the
  ledger is not at `./.nether`.
- `g:nether_scratch_buf` — buffer number of the unnamed `nofile` scratch window
  created to display hover and provenance reports.

## Usage

1. Bury (and optionally exhume) a `.nc` file:

   ```sh
   nether bury build.nc
   ```

2. Bind the trace cairn to the open buffer:

   ```vim
   :NetherTrace 76c4b655
   ```

3. Commands and buffer-local normal-mode mappings:

   - `K` / `:NetherHover` — show the expression's derived depth `0..8`, type,
     enclosing or responsible `descend κ` span, deepest call span, and the
     provenance walk of any recorded `Deposit`, `Hole` or `Witness` node at the
     cursor in the unnamed scratch buffer (`g:nether_scratch_buf`).
   - `gd` / `:NetherOrigin` — jump to the `descend κ` responsible for the
     expression's depth; pressing `gd` on that `descend` jumps to the call
     inside it.
   - `gp` / `:NetherProvenance` — open the provenance walk in the unnamed
     scratch buffer (`g:nether_scratch_buf`) and jump to the recorded
     world-reaching call.

If the buffer has been edited since the trace was buried, `nether lamp` refuses
with exit code 1, naming both short cairns rather than guessing where the lines
moved. A stale trace does not get a polite approximation; bury the buffer again.
