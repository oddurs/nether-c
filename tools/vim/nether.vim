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
  if line('$') == 1 && empty(getline(1)) && !&eol
    return 0
  endif
  let l:bom = &bomb ? 3 : 0
  let l:off = line2byte(line('.')) + col('.') - 2 + l:bom
  return l:off < 0 ? 0 : l:off
endfunction

function! s:WriteBuffer(tmp) abort
  if !empty(&fileencoding) && &fileencoding !=# 'utf-8'
    throw 'nether: buffer fileencoding must be utf-8 (got ' . &fileencoding . ')'
  endif
  let l:lines = getline(1, '$')
  if &encoding !=# 'utf-8'
    let l:lines = map(copy(l:lines), 'iconv(v:val, &encoding, "utf-8")')
  endif
  if line('$') == 1 && empty(getline(1)) && !&bomb && !&eol
    call writefile([], a:tmp, 'b')
    return
  endif
  if &fileformat ==# 'mac'
    let l:body = join(l:lines, "\r") . (&eol ? "\r" : '')
    if &bomb
      let l:body = "\xEF\xBB\xBF" . l:body
    endif
    call writefile([l:body], a:tmp, 'b')
    return
  endif
  if &fileformat ==# 'dos'
    if &eol
      let l:lines = map(copy(l:lines), 'v:val . "\r"')
    elseif len(l:lines) > 1
      let l:lines = map(copy(l:lines[0:-2]), 'v:val . "\r"') + [l:lines[-1]]
    endif
  endif
  if &bomb && !empty(l:lines)
    let l:lines[0] = "\xEF\xBB\xBF" . l:lines[0]
  endif
  call writefile(l:lines, a:tmp, &eol ? '' : 'b')
endfunction

function! s:FormatReport(data) abort
  let l:lines = [printf('depth %d   %s   %s   at %d..%d',
        \ a:data.depth, a:data.stratum, a:data.type,
        \ a:data.span.start, a:data.span.end)]
  if type(get(a:data, 'descend', v:null)) == v:t_dict
    call add(l:lines, printf('descend %s   at %d..%d',
          \ a:data.descend.capability, a:data.descend.start, a:data.descend.end))
  endif
  if type(get(a:data, 'call', v:null)) == v:t_dict
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

function! s:ClearScratch() abort
  if s:scratch_buf > 0 && bufexists(s:scratch_buf)
    call setbufvar(s:scratch_buf, '&modifiable', 1)
    silent call deletebufline(s:scratch_buf, 1, '$')
    call setbufvar(s:scratch_buf, '&modifiable', 0)
  endif
endfunction

function! s:ShowScratch(lines) abort
  let l:src_win = win_getid()
  let l:buf = (s:scratch_buf > 0 && bufexists(s:scratch_buf))
        \ ? s:scratch_buf
        \ : bufadd('')
  let s:scratch_buf = l:buf
  let g:nether_scratch_buf = l:buf
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

function! s:Error(msg) abort
  let g:nether_last_error = a:msg
  echohl ErrorMsg | echom g:nether_last_error | echohl None
endfunction

function! s:FieldStart(data, key) abort
  let l:field = get(a:data, a:key, v:null)
  return type(l:field) == v:t_dict ? l:field.start : -1
endfunction

function! s:GotoByte(byte) abort
  let l:bom = &bomb ? 3 : 0
  execute 'goto ' . max([1, a:byte - l:bom + 1])
endfunction

function! s:FailInspect(msg) abort
  let g:nether_last_lamp = []
  let g:nether_last_json = {}
  call s:ClearScratch()
  call s:Error(a:msg)
  return 0
endfunction

function! s:Inspect() abort
  if !exists('b:nether_trace') || empty(b:nether_trace)
    return s:FailInspect('nether: no trace bound; bind one with :NetherTrace <cairn>')
  endif
  let l:byte = s:ByteOffset()
  let l:tmp = tempname()
  let l:err = tempname()
  let l:out = ''
  let l:err_text = ''
  try
    call s:WriteBuffer(l:tmp)
    let l:cmd = shellescape(s:Bin()) . ' lamp --at ' . l:byte
          \ . ' --source ' . shellescape(l:tmp)
          \ . ' --json -- ' . shellescape(b:nether_trace)
          \ . ' 2>' . shellescape(l:err)
    let l:out = system(l:cmd)
  catch
    return s:FailInspect(v:exception)
  finally
    call delete(l:tmp)
    if filereadable(l:err)
      let l:err_text = join(readfile(l:err), "\n")
      call delete(l:err)
    endif
  endtry
  if v:shell_error != 0
    return s:FailInspect(trim(!empty(l:err_text) ? l:err_text : l:out))
  endif
  try
    let l:data = json_decode(l:out)
  catch
    return s:FailInspect('nether: invalid JSON from lamp')
  endtry
  if type(l:data) != v:t_dict
        \ || !has_key(l:data, 'depth')
        \ || !has_key(l:data, 'stratum')
        \ || !has_key(l:data, 'type')
        \ || type(get(l:data, 'span', v:null)) != v:t_dict
    return s:FailInspect('nether: invalid JSON from lamp')
  endif
  let g:nether_last_error = ''
  let g:nether_last_json = l:data
  let g:nether_last_lamp = s:FormatReport(l:data)
  call s:ShowScratch(g:nether_last_lamp)
  return 1
endfunction

function! s:BindTrace(cairn) abort
  if a:cairn !~# '^[0-9a-f]\{1,64\}$'
    call s:Error('nether: invalid trace cairn: ' . a:cairn)
    return
  endif
  let b:nether_trace = a:cairn
  if !get(g:, 'nether_no_maps', 0)
    nnoremap <buffer> <silent> K :NetherHover<CR>
    nnoremap <buffer> <silent> gd :NetherOrigin<CR>
    nnoremap <buffer> <silent> gp :NetherProvenance<CR>
  endif
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
  let l:desc = s:FieldStart(l:data, 'descend')
  let l:call = s:FieldStart(l:data, 'call')
  if l:desc >= 0 && l:here != l:desc
    call s:GotoByte(l:desc)
  elseif l:call >= 0
    call s:GotoByte(l:call)
  elseif l:desc >= 0
    echomsg 'nether: already at descend origin'
  else
    echomsg 'nether: no descend or call origin at cursor'
  endif
endfunction

function! s:Provenance() abort
  if !s:Inspect()
    return
  endif
  let l:data = get(g:, 'nether_last_json', {})
  let l:call = s:FieldStart(l:data, 'call')
  if l:call >= 0
    call s:GotoByte(l:call)
  else
    echomsg 'nether: no call origin at cursor'
  endif
endfunction

command! -nargs=1 NetherTrace call s:BindTrace(<q-args>)
command! -nargs=0 NetherHover call s:Hover()
command! -nargs=0 NetherOrigin call s:Origin()
command! -nargs=0 NetherProvenance call s:Provenance()
