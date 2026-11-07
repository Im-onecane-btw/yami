# One password for the whole install.
#
# Every root command goes to one root shell started once (root_start): sudo or doas asks a single
# time, even doas without `persist` and even when a step falls back to installing packages one by
# one (it used to ask for each). The cached ticket is dropped right after, so nothing else run by
# the install (the crates' build scripts, minutes of them) can simply call sudo; the channel to the
# root shell is two pipes bash does not pass to the programs it starts ({var} descriptors). They
# are not secret from this user: a process of the same user can still reach them through
# /proc/PID/fd while they are open, so the root shell lives only through the root steps, which all
# come first, and is closed before anything is compiled (run_steps → root_stop before the build).
# A command runs with the caller's stdin, stdout and stderr (through /proc), so pipes into `tee`
# and the log work as before (a socket cannot be reopened: /dev/null instead); it returns the
# command's exit code.
# shellcheck shell=bash
ROOT_W="" ROOT_R="" ROOT_TOOL=""
root_start() {
  local tool=$SU
  [[ -n $tool ]] || return 0
  ROOT_TOOL=$tool
  coproc ROOTSH {
    # shellcheck disable=SC2016
    "$tool" bash -c 'while IFS= read -r p && IFS= read -r c; do
      i=/proc/$p/fd/0 o=/proc/$p/fd/1 e=/proc/$p/fd/2
      { : <"$i"; } 2>/dev/null || i=/dev/null
      { : >>"$o"; } 2>/dev/null || o=/dev/null
      { : >>"$e"; } 2>/dev/null || e=/dev/null
      eval "$c" <"$i" >>"$o" 2>>"$e"; echo "$?"
    done'
  }
  local w=${ROOTSH[1]} r=${ROOTSH[0]}
  exec {ROOT_W}>&"$w" {ROOT_R}<&"$r"
  # only the copies stay: closing them is the root shell's end of input
  eval "exec $w>&- $r<&-"
  SU=asroot
  # the password prompt happens here, on the terminal; a wrong one ends the root shell
  asroot true || { root_stop; SU=$tool; return 1; }
  case $tool in sudo) sudo -k ;; doas) doas -L 2>/dev/null ;; esac
  return 0
}
asroot() {
  local q rc me=$BASHPID
  [[ -n $ROOT_W ]] || return 1
  printf -v q '%q ' "$@"
  # the root shell opens this process's 0, 1 and 2 while it waits: a redirection here (`>&fd`,
  # `<&fd`) would swap them for that moment, so the request is written from a subshell and the
  # answer read with -u. A root shell that is gone must not take the installer with it (SIGPIPE).
  if ! (trap '' PIPE; printf '%s\n%s\n' "$me" "$q" >&"$ROOT_W") 2>/dev/null || ! IFS= read -r -u "$ROOT_R" rc; then
    printf '%s\n' "$(t "root-сеанс закрыт" "the root shell is gone")" >&2
    return 1
  fi
  return "$rc"
}
root_stop() {
  [[ -n $ROOT_W ]] && exec {ROOT_W}>&- {ROOT_R}<&-
  ROOT_W="" ROOT_R=""
}
