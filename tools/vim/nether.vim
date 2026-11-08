" tools/vim/nether.vim — inspect a buried Nether C trace from Vim/Neovim.
"
" Calls `nether lamp <cairn> --at <byte> --source <tmp>` (`spec/08-rites.md`
" §8.4.1). No depth rules or spans are computed in Vimscript.

if exists('g:loaded_nether_vim')
  finish
endif
let g:loaded_nether_vim = 1

function! s:Bin() abort
  return get(g:, 'nether_bin', 'nether')
endfunction

function! s:ByteOffset() abort
  return line2byte(line('.')) + col('.') - 2
endfunction

function! s:ShowScratch(lines) abort
  let l:src_win = win_getid()
  let l:buf = bufnr('__nether_lamp__', 1)
  call bufload(l:buf)
  call setbufvar(l:buf, '&buftype', 'nofile')
  call setbufvar(l:buf, '&bufhidden', 'hide')
  call setbufvar(l:buf, '&swapfile', 0)
  call setbufvar(l:buf, '&buflisted', 0)
  call setbufvar(l:buf, '&modifiable', 1)
  silent call deletebufline(l:buf, 1, '$')
  call setbufline(l:buf, 1, a:lines)
  call setbufvar(l:buf, '&modifiable', 0)
  if bufwinid(l:buf) == -1
    let l:wins = winnr('$')
    silent! keepalt botright 8split
    if winnr('$') > l:wins
      execute 'silent keepalt buffer ' . l:buf
      call win_gotoid(l:src_win)
    endif
  endif
endfunction

function! s:Inspect() abort
  if !exists('b:nether_trace') || empty(b:nether_trace)
    let g:nether_last_error = 'nether: no trace bound; run :NetherTrace <cairn>'
    echohl ErrorMsg | echom g:nether_last_error | echohl None
    return 0
  endif
  let l:byte = s:ByteOffset()
  let l:tmp = tempname()
  call writefile(getline(1, '$'), l:tmp, &eol ? '' : 'b')
  let l:cmd = shellescape(s:Bin()) . ' lamp ' . shellescape(b:nether_trace)
        \ . ' --at ' . l:byte . ' --source ' . shellescape(l:tmp)
  let l:out = system(l:cmd)
  call delete(l:tmp)
  if v:shell_error != 0
    let g:nether_last_error = get(split(l:out, "\n"), 0, '')
    let g:nether_last_lamp = []
    echohl ErrorMsg | echom g:nether_last_error | echohl None
    return 0
  endif
  let g:nether_last_error = ''
  let g:nether_last_lamp = split(l:out, "\n")
  call s:ShowScratch(g:nether_last_lamp)
  return 1
endfunction

function! s:SpanStart(prefix) abort
  for l:line in get(g:, 'nether_last_lamp', [])
    let l:m = matchlist(l:line, '^' . a:prefix . ' .\+   at \(\d\+\)\.\.\d\+$')
    if !empty(l:m)
      return str2nr(l:m[1])
    endif
  endfor
  return -1
endfunction

function! s:BindTrace(cairn) abort
  let b:nether_trace = a:cairn
  nnoremap <buffer> <silent> K :NetherHover<CR>
  nnoremap <buffer> <silent> gd :NetherOrigin<CR>
  nnoremap <buffer> <silent> gp :NetherProvenance<CR>
endfunction

function! s:Hover() abort
  call s:Inspect()
endfunction

function! s:Origin() abort
  let l:here = s:ByteOffset()
  if !s:Inspect()
    return
  endif
  let l:desc = s:SpanStart('descend')
  let l:call = s:SpanStart('call')
  if l:desc >= 0 && l:here != l:desc
    execute 'goto ' . (l:desc + 1)
  elseif l:call >= 0
    execute 'goto ' . (l:call + 1)
  endif
endfunction

function! s:Provenance() abort
  if !s:Inspect()
    return
  endif
  let l:call = s:SpanStart('call')
  if l:call >= 0
    execute 'goto ' . (l:call + 1)
  endif
endfunction

command! -nargs=1 NetherTrace call s:BindTrace(<q-args>)
command! -nargs=0 NetherHover call s:Hover()
command! -nargs=0 NetherOrigin call s:Origin()
command! -nargs=0 NetherProvenance call s:Provenance()
