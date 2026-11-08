# `tools/vim/nether.vim`

Thin Vim/Neovim adapter for [`nether lamp <cairn> --at <byte>`](../../spec/08-rites.md#841-inspecting-a-byte-offset-in-an-editor).

## Usage

1. Bury (and optionally exhume) a `.nc` file:

   ```sh
   nether bury build.nc
   ```

2. Load the adapter in Vim or Neovim and bind the trace cairn to the buffer:

   ```vim
   :source tools/vim/nether.vim
   :NetherTrace 76c4b655
   ```

3. Commands and buffer-local normal-mode mappings:

   - `K` / `:NetherHover` — show the expression's derived depth `0..8`, type,
     enclosing or responsible `descend κ` span, deepest call span, and the
     provenance walk of any recorded `Deposit`, `Hole` or `Witness` node at the
     cursor in the `__nether_lamp__` scratch window.
   - `gd` / `:NetherOrigin` — jump to the `descend κ` responsible for the
     expression's depth; pressing `gd` on that `descend` jumps to the call
     inside it.
   - `gp` / `:NetherProvenance` — open the provenance walk in
     `__nether_lamp__` and jump to the recorded world-reaching call.

If the buffer has been edited since the trace was buried, `nether lamp` refuses
with exit code 1 naming both short cairns rather than guessing shifted offsets.
