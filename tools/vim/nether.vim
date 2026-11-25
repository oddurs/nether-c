" tools/vim/nether.vim — inspect a buried Nether C trace from Vim/Neovim.
"
" Calls `nether lamp <cairn> --at <byte> --source <tmp> --json`
" (`spec/08-rites.md` §8.4.1). No depth rules or spans are computed in Vimscript.

if exists('g:loaded_nether_vim')
  finish
endif
let g:loaded_nether_vim = 1

let s:scratch_buf = -1

function! s:Bin() abort
  return get(g:, 'nether_bin', 'nether')
endfunction

function! s:ByteOffset() abort
  if line('$') == 1 && empty(getline(1))
    return 0
  endif
  let l:bom = &bomb ? 3 : 0
  let l:off = line2byte(line('.')) + col('.') - 2 + l:bom
  return l:off < 0 ? 0 : l:off
endfunction

function! s:WriteBuffer(tmp) abort
  let l:lines = getline(1, '$')
  if &fileformat ==# 'dos'
    let l:lines = map(copy(l:lines), 'v:val . "\r"')
  endif
  if &bomb && !empty(l:lines)
    let l:lines[0] = "\xEF\xBB\xBF" . l:lines[0]
  endif
  if line('$') == 1 && empty(getline(1)) && !&bomb
    call writefile([], a:tmp, 'b')
    return
  endif
  call writefile(l:lines, a:tmp, &eol ? '' : 'b')
endfunction

function! s:FormatReport(data) abort
  let l:lines = [printf('depth %d   %s   %s   at %d..%d',
        \ a:data.depth, a:data.stratum, a:data.type,
        \ a:data.span.start, a:data.span.end)]
  if type(a:data.descend) == v:t_dict
    call add(l:lines, printf('descend %s   at %d..%d',
          \ a:data.descend.capability, a:data.descend.start, a:data.descend.end))
  endif
  if type(a:data.call) == v:t_dict
    call add(l:lines, printf('call %s   at %d..%d',
          \ a:data.call.name, a:data.call.start, a:data.call.end))
  endif
  for l:node in get(a:data, 'nodes', [])
    for l:line in get(l:node, 'provenance', [])
      call add(l:lines, l:line)
    endfor
  endfor
  return l:lines
endfunction

function! s:ShowScratch(lines) abort
  let l:src_win = win_getid()
  let l:buf = (s:scratch_buf > 0 && bufexists(s:scratch_buf))
        \ ? s:scratch_buf
        \ : bufnr('^__nether_lamp__$', 1)
  let s:scratch_buf = l:buf
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
  let l:out = ''
  try
    call s:WriteBuffer(l:tmp)
    let l:cmd = shellescape(s:Bin()) . ' lamp --at ' . l:byte
          \ . ' --source ' . shellescape(l:tmp)
          \ . ' --json -- ' . shellescape(b:nether_trace)
    let l:out = system(l:cmd)
  finally
    call delete(l:tmp)
  endtry
  if v:shell_error != 0
    let g:nether_last_error = trim(l:out)
    let g:nether_last_lamp = []
    let g:nether_last_json = {}
    echohl ErrorMsg | echom g:nether_last_error | echohl None
    return 0
  endif
  try
    let l:data = json_decode(l:out)
  catch
    let g:nether_last_error = 'nether: invalid JSON from lamp'
    let g:nether_last_lamp = []
    let g:nether_last_json = {}
    echohl ErrorMsg | echom g:nether_last_error | echohl None
    return 0
  endtry
  let g:nether_last_error = ''
  let g:nether_last_json = l:data
  let g:nether_last_lamp = s:FormatReport(l:data)
  call s:ShowScratch(g:nether_last_lamp)
  return 1
endfunction

function! s:BindTrace(cairn) abort
  if a:cairn !~# '^[0-9a-f]\{1,64\}$'
    let g:nether_last_error = 'nether: invalid trace cairn: ' . a:cairn
    echohl ErrorMsg | echom g:nether_last_error | echohl None
    return
  endif
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
  let l:data = get(g:, 'nether_last_json', {})
  let l:bom = &bomb ? 3 : 0
  let l:desc = type(get(l:data, 'descend', v:null)) == v:t_dict ? l:data.descend.start : -1
  let l:call = type(get(l:data, 'call', v:null)) == v:t_dict ? l:data.call.start : -1
  if l:desc >= 0 && l:here != l:desc
    execute 'goto ' . (l:desc - l:bom + 1)
  elseif l:call >= 0
    execute 'goto ' . (l:call - l:bom + 1)
  endif
endfunction

function! s:Provenance() abort
  if !s:Inspect()
    return
  endif
  let l:data = get(g:, 'nether_last_json', {})
  let l:bom = &bomb ? 3 : 0
  let l:call = type(get(l:data, 'call', v:null)) == v:t_dict ? l:data.call.start : -1
  if l:call >= 0
    execute 'goto ' . (l:call - l:bom + 1)
  endif
endfunction

command! -nargs=1 NetherTrace call s:BindTrace(<q-args>)
command! -nargs=0 NetherHover call s:Hover()
command! -nargs=0 NetherOrigin call s:Origin()
command! -nargs=0 NetherProvenance call s:Provenance()
